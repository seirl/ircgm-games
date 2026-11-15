//! Anti-Timing-Attack **Deck-Derived Fixed-Clock Tier Scheduler** for Dusk & Night.
//!
//! - [`build_deck_schedule`] derives the complete sequence of [`ScheduledTier`]s and their exact
//!   wall-clock offsets **purely from the public `deck` multiset** (never from dealt seats, center
//!   cards, copycat targets, `Mark::Fear`, or player response speed).
//! - Every card in a tier is called at `origin + tier.start_offset`, and [`run_night_phase`]
//!   unconditionally waits until `origin + tier.end_offset` (`tokio::time::sleep_until`) before
//!   committing mutations, sending passive/third-party DMs, or advancing to the next tier.

use std::collections::HashSet;
use std::time::Duration;

use futures::future::try_join_all;
use ircgm::prelude::*;
use tokio::time::{sleep_until, Instant};

use crate::board::{BoardCards, Roster};
use crate::marks::Mark;
use crate::role::{
    BoxFuture, CopiedRecord, CopyBehavior, GroupEffect, GroupPromptCtx, PassiveWakeFn, PromptCtx,
    ResolveCtx, Role, RoleTag, SeatEffect, WakeKind, WakePhase,
};
use crate::roles::ALL_ROLES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NightTimings {
    /// Fixed wall-clock duration of every standard Dusk or Night role tier.
    pub tier_budget: Duration,
    /// Fixed wall-clock duration of a Copycat / Doppelgänger card-selection sub-tier.
    pub copycat_pick_budget: Duration,
    /// Optional countdown warning before a tier deadline expires.
    pub warn_remaining: Option<Duration>,
}

impl Default for NightTimings {
    fn default() -> Self {
        Self {
            tier_budget: Duration::from_secs(25),
            copycat_pick_budget: Duration::from_secs(15),
            warn_remaining: Some(Duration::from_secs(5)),
        }
    }
}

impl NightTimings {
    pub fn fast_for_tests() -> Self {
        Self {
            tier_budget: Duration::from_secs(5),
            copycat_pick_budget: Duration::from_secs(3),
            warn_remaining: None,
        }
    }

    fn make_tier_cfg(&self, deadline: Instant) -> AskConfig {
        let mut cfg = AskConfig::new()
            .reply_deadline(deadline)
            .ignore_commands()
            .timeout_message("⏳ Time's up!".yellow());
        if let Some(warn_rem) = self.warn_remaining {
            cfg = cfg.warn_remaining(warn_rem, "⏳ Hurry—time is almost up!".yellow());
        }
        cfg
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TierPhase {
    Dusk,
    Night,
}

#[derive(Clone, Debug)]
pub enum TierKind {
    /// Standard concurrent tier of Dusk or Night roles (resolved in ascending `wake.order`).
    Roles(Vec<&'static dyn Role>),
    /// Copycat / Doppelgänger Step 1: pick a center or player card to copy.
    CopycatPick(&'static dyn Role),
    /// Copycat / Doppelgänger Step 2: fixed-clock window for `CopyBehavior::ActImmediately` roles
    /// (present iff `deck` contains at least one `ActImmediately` role, regardless of what was copied).
    CopycatImmediate(&'static dyn Role),
}

#[derive(Clone, Debug)]
pub struct ScheduledTier {
    pub phase: TierPhase,
    pub kind: TierKind,
    pub duration: Duration,
    pub start_offset: Duration,
    pub end_offset: Duration,
}

impl ScheduledTier {
    pub fn callout_label(&self) -> String {
        match &self.kind {
            TierKind::Roles(roles) => roles
                .iter()
                .map(|r| r.name())
                .collect::<Vec<_>>()
                .join(", "),
            TierKind::CopycatPick(r) => r.name().to_owned(),
            TierKind::CopycatImmediate(r) => format!("{} (copied action)", r.name()),
        }
    }

    pub fn styled_callout(&self) -> Text {
        match &self.kind {
            TierKind::Roles(roles) => {
                let parts: Vec<Text> = roles.iter().map(|r| r.styled_name()).collect();
                Text::join(&parts, ", ".grey())
            }
            TierKind::CopycatPick(r) => r.styled_name(),
            TierKind::CopycatImmediate(r) => {
                irc_format!("{} {}", r.styled_name(), "(copied action)".grey())
            }
        }
    }
}

/// Pure function of the public `deck` multiset and `timings` — any permutation of the same
/// `(N + 3)` cards produces the exact same tier list and wall-clock offsets.
pub fn build_deck_schedule(
    deck: &[&'static dyn Role],
    timings: &NightTimings,
) -> Vec<ScheduledTier> {
    let mut waking: Vec<&'static dyn Role> = Vec::new();
    let mut push_unique = |r: &'static dyn Role| {
        if r.wake().is_some() && !waking.iter().any(|e| e.id() == r.id()) {
            waking.push(r);
        }
    };
    for &r in deck {
        for &tag in r.tags() {
            if let Some(&pack_role) = ALL_ROLES.iter().find(|cand| {
                cand.wake().is_some_and(|w| {
                    matches!(w.kind, WakeKind::Group { pack: Some(t), .. } if t == tag)
                })
            }) {
                push_unique(pack_role);
            }
        }
        push_unique(r);
    }
    waking.sort_by_key(|r| {
        let w = r.wake().expect("filtered to waking roles");
        (w.phase, w.order, r.id())
    });

    let in_phase = |phase: WakePhase| -> Vec<&'static dyn Role> {
        waking
            .iter()
            .copied()
            .filter(|r| r.wake().is_some_and(|w| w.phase == phase))
            .collect()
    };

    let pre_copy_roles = in_phase(WakePhase::PreCopycatDusk);
    let copycat_roles = in_phase(WakePhase::Copycat);
    let dusk_roles = in_phase(WakePhase::Dusk);
    let night_roles = in_phase(WakePhase::Night);

    let has_dusk = !pre_copy_roles.is_empty() || !dusk_roles.is_empty();
    let copycat_phase = if has_dusk {
        TierPhase::Dusk
    } else {
        TierPhase::Night
    };
    let deck_has_act_immediately = deck
        .iter()
        .any(|r| !r.is_copycat() && r.copy_behavior() == CopyBehavior::ActImmediately);

    let mut raw_tiers: Vec<(TierPhase, TierKind, Duration)> = Vec::new();

    if !pre_copy_roles.is_empty() {
        raw_tiers.push((
            TierPhase::Dusk,
            TierKind::Roles(pre_copy_roles),
            timings.tier_budget,
        ));
    }

    for cc in copycat_roles {
        raw_tiers.push((
            copycat_phase,
            TierKind::CopycatPick(cc),
            timings.copycat_pick_budget,
        ));
        if deck_has_act_immediately {
            raw_tiers.push((
                copycat_phase,
                TierKind::CopycatImmediate(cc),
                timings.tier_budget,
            ));
        }
    }

    for group in partition_role_tiers(&dusk_roles) {
        raw_tiers.push((TierPhase::Dusk, TierKind::Roles(group), timings.tier_budget));
    }

    for group in partition_role_tiers(&night_roles) {
        raw_tiers.push((
            TierPhase::Night,
            TierKind::Roles(group),
            timings.tier_budget,
        ));
    }

    let mut cursor = Duration::ZERO;
    raw_tiers
        .into_iter()
        .map(|(phase, kind, duration)| {
            let start_offset = cursor;
            let end_offset = start_offset + duration;
            cursor = end_offset;
            ScheduledTier {
                phase,
                kind,
                duration,
                start_offset,
                end_offset,
            }
        })
        .collect()
}

fn partition_role_tiers(roles: &[&'static dyn Role]) -> Vec<Vec<&'static dyn Role>> {
    let mut tiers: Vec<Vec<&'static dyn Role>> = Vec::new();
    let mut cur: Vec<&'static dyn Role> = Vec::new();
    let mut cur_has_mutator = false;

    for &role in roles {
        let wake = role.wake().expect("filtered to waking roles");
        let shares_pack_with_tier = cur.iter().any(|prev| match prev.wake().map(|w| w.kind) {
            Some(WakeKind::Group {
                pack: Some(pack_tag),
                ..
            }) => role.tags().contains(&pack_tag),
            _ => false,
        });
        let must_split = !cur.is_empty()
            && wake.needs_dm_prompt()
            && (shares_pack_with_tier || (wake.reads_live_board && cur_has_mutator));

        if must_split {
            tiers.push(std::mem::take(&mut cur));
            cur_has_mutator = false;
        }
        if wake.mutates_board {
            cur_has_mutator = true;
        }
        cur.push(role);
    }

    if !cur.is_empty() {
        tiers.push(cur);
    }
    tiers
}

// ============================================================================
// Fixed-Clock Tier Execution Engine
// ============================================================================

enum CopiedImmediateOutcome {
    Passive(PassiveWakeFn),
    Solo(SeatEffect),
}

enum RoleTierResolution {
    Group(GroupEffect),
    Passive {
        seat: usize,
        label: &'static str,
        wake_fn: PassiveWakeFn,
    },
    Solo {
        label: &'static str,
        effect: SeatEffect,
    },
}

pub async fn run_night_phase(
    bot: &Bot,
    room: &Room,
    roster: &Roster,
    mut board: BoardCards,
    timings: &NightTimings,
) -> Result<(BoardCards, Vec<Text>)> {
    let mut all_chronicle: Vec<Text> = Vec::new();

    for (seat, info) in roster.seats.iter().enumerate() {
        let mut rctx = ResolveCtx {
            roster,
            chronicle: &mut all_chronicle,
        };
        info.initial_role.on_deal(seat, &mut board, &mut rctx);
    }

    let schedule = build_deck_schedule(&roster.deck, timings);
    if schedule.is_empty() {
        room.say("🌙 Night falls over a peaceful village...".bold().light_blue())?;
        return Ok((board, all_chronicle));
    }

    let origin = Instant::now();
    let total_tiers = schedule.len();
    let mut current_phase: Option<TierPhase> = None;
    let mut copied_records: Vec<CopiedRecord> = Vec::new();
    let mut feared_seats: HashSet<usize> = HashSet::new();
    let mut dusk_finalized = false;

    for (tier_idx, tier) in schedule.iter().enumerate() {
        if current_phase == Some(TierPhase::Dusk) && tier.phase == TierPhase::Night && !dusk_finalized
        {
            finalize_dusk_boundary(
                roster,
                &copied_records,
                &mut board,
                &mut feared_seats,
                &mut all_chronicle,
            );
            dusk_finalized = true;
        }

        if current_phase != Some(tier.phase) {
            match tier.phase {
                TierPhase::Dusk => {
                    room.say("🌆 Dusk falls! Dusk roles check your DMs...".bold().orange())?;
                }
                TierPhase::Night => {
                    room.say(
                        "🌙 Night falls! Active roles check your DMs..."
                            .bold()
                            .light_blue(),
                    )?;
                }
            }
            current_phase = Some(tier.phase);
        }

        let tier_deadline = origin + tier.end_offset;
        let tier_cfg = timings.make_tier_cfg(tier_deadline);

        room.say(irc_format!(
            "  {} Calling: {}",
            format!("Tier {}/{total_tiers} ({}s) ·", tier_idx + 1, tier.duration.as_secs()).grey(),
            tier.styled_callout(),
        ))?;

        match &tier.kind {
            TierKind::CopycatPick(role) => {
                let WakeKind::Copycat { pick, .. } = role.wake().expect("copycat wake").kind else {
                    unreachable!("CopycatPick tier requires WakeKind::Copycat");
                };
                let holders: Vec<usize> = roster
                    .initial_holders(*role)
                    .into_iter()
                    .filter(|s| !feared_seats.contains(s))
                    .collect();

                let board_snap = &board;
                let shielded_snap = &board.shielded;
                let copied_snap = &copied_records;
                let cfg_ref = &tier_cfg;
                let futs = holders.iter().map(|&seat| async move {
                    let dm = roster.seats[seat].user.exclusive_available().await?;
                    let pctx = PromptCtx {
                        seat,
                        dm,
                        roster,
                        ask_cfg: cfg_ref,
                        deadline: tier_deadline,
                        shielded: shielded_snap,
                        copied_records: copied_snap,
                        live_board: Some(board_snap),
                    };
                    pick(seat, pctx).await
                });

                let effects = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for effect in effects {
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                    };
                    if let Some(rec) = effect.apply(&mut board, &mut rctx) {
                        copied_records.push(rec);
                    }
                }
            }

            TierKind::CopycatImmediate(role) => {
                let immediate_targets: Vec<CopiedRecord> = copied_records
                    .iter()
                    .copied()
                    .filter(|rec| {
                        roster.seats[rec.copycat_seat].initial_role.id() == role.id()
                            && !feared_seats.contains(&rec.copycat_seat)
                            && rec.copied_role.copy_behavior() == CopyBehavior::ActImmediately
                    })
                    .collect();

                let board_snap = &board;
                let shielded_snap = &board.shielded;
                let copied_snap = &copied_records;
                let cfg_ref = &tier_cfg;
                let futs = immediate_targets.iter().map(|&rec| async move {
                    let wake = rec.copied_role.wake().expect("immediate copied role wakes");
                    match wake.kind {
                        WakeKind::Passive(wake_fn) => {
                            Ok::<_, Error>((rec, CopiedImmediateOutcome::Passive(wake_fn)))
                        }
                        WakeKind::Solo(prompt_fn) => {
                            let dm = roster.seats[rec.copycat_seat]
                                .user
                                .exclusive_available()
                                .await?;
                            let pctx = PromptCtx {
                                seat: rec.copycat_seat,
                                dm,
                                roster,
                                ask_cfg: cfg_ref,
                                deadline: tier_deadline,
                                shielded: shielded_snap,
                                copied_records: copied_snap,
                                live_board: wake.reads_live_board.then_some(board_snap),
                            };
                            let effect = prompt_fn(rec.copycat_seat, pctx).await?;
                            Ok((rec, CopiedImmediateOutcome::Solo(effect)))
                        }
                        _ => unreachable!("ActImmediately only applies to Solo or Passive roles"),
                    }
                });

                let outcomes = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for (rec, outcome) in outcomes {
                    let base_name = roster.seats[rec.copycat_seat].initial_role.name();
                    let label = format!("{base_name}-{}", rec.copied_role.name());
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                    };
                    match outcome {
                        CopiedImmediateOutcome::Passive(wake_fn) => {
                            wake_fn(rec.copycat_seat, &label, &mut board, &mut rctx);
                        }
                        CopiedImmediateOutcome::Solo(effect) => {
                            effect.apply(&label, &mut board, &mut rctx);
                        }
                    }
                }
            }

            TierKind::Roles(roles) => {
                let board_snap = &board;
                let shielded_snap = &board.shielded;
                let copied_snap = &copied_records;
                let feared_snap = &feared_seats;
                let cfg_ref = &tier_cfg;

                let mut futs: Vec<BoxFuture<'_, Result<RoleTierResolution>>> = Vec::new();

                for &role in roles {
                    let wake = role.wake().expect("scheduled role has Wake metadata");
                    match wake.kind {
                        WakeKind::Group { pack, prompt } => {
                            let active_seats = compute_group_active_seats(
                                roster,
                                role,
                                pack,
                                copied_snap,
                                feared_snap,
                            );
                            if active_seats.is_empty() {
                                continue;
                            }
                            futs.push(Box::pin(async move {
                                let gctx = GroupPromptCtx {
                                    bot,
                                    roster,
                                    ask_cfg: cfg_ref,
                                    deadline: tier_deadline,
                                    copied_records: copied_snap,
                                    shielded: shielded_snap,
                                    feared_seats: feared_snap,
                                    live_board: wake.reads_live_board.then_some(board_snap),
                                };
                                let effect = prompt(active_seats, gctx).await?;
                                Ok(RoleTierResolution::Group(effect))
                            }));
                        }
                        WakeKind::Passive(wake_fn) => {
                            let label = role.name();
                            for seat in roster
                                .initial_holders(role)
                                .into_iter()
                                .filter(|s| !feared_snap.contains(s))
                            {
                                futs.push(Box::pin(async move {
                                    Ok(RoleTierResolution::Passive {
                                        seat,
                                        label,
                                        wake_fn,
                                    })
                                }));
                            }
                        }
                        WakeKind::Solo(prompt_fn) => {
                            let label = role.name();
                            for seat in roster
                                .initial_holders(role)
                                .into_iter()
                                .filter(|s| !feared_snap.contains(s))
                            {
                                futs.push(Box::pin(async move {
                                    let dm = roster.seats[seat].user.exclusive_available().await?;
                                    let pctx = PromptCtx {
                                        seat,
                                        dm,
                                        roster,
                                        ask_cfg: cfg_ref,
                                        deadline: tier_deadline,
                                        shielded: shielded_snap,
                                        copied_records: copied_snap,
                                        live_board: wake.reads_live_board.then_some(board_snap),
                                    };
                                    let effect = prompt_fn(seat, pctx).await?;
                                    Ok(RoleTierResolution::Solo { label, effect })
                                }));
                            }
                        }
                        WakeKind::Copycat { .. } => {
                            unreachable!("Copycat roles are scheduled via TierKind::CopycatPick")
                        }
                    }
                }

                let resolutions = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for res in resolutions {
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                    };
                    match res {
                        RoleTierResolution::Group(effect) => {
                            effect.apply(&mut board, &mut rctx);
                        }
                        RoleTierResolution::Passive {
                            seat,
                            label,
                            wake_fn,
                        } => {
                            wake_fn(seat, label, &mut board, &mut rctx);
                        }
                        RoleTierResolution::Solo { label, effect } => {
                            effect.apply(label, &mut board, &mut rctx);
                        }
                    }
                }
            }
        }
    }

    if current_phase == Some(TierPhase::Dusk) && !dusk_finalized {
        finalize_dusk_boundary(
            roster,
            &copied_records,
            &mut board,
            &mut feared_seats,
            &mut all_chronicle,
        );
    }

    let tail_entries = run_tail_copycats(
        roster,
        &copied_records,
        &feared_seats,
        TierPhase::Night,
        &mut board,
    );
    all_chronicle.extend(tail_entries);

    Ok((board, all_chronicle))
}

fn compute_group_active_seats(
    roster: &Roster,
    role: &'static dyn Role,
    tag_opt: Option<RoleTag>,
    copied_records: &[CopiedRecord],
    feared_seats: &HashSet<usize>,
) -> Vec<usize> {
    let mut active_seats: Vec<usize> = if let Some(tag) = tag_opt {
        let mut seats: Vec<usize> = roster
            .seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                (s.initial_role.tags().contains(&tag) && !feared_seats.contains(&i)).then_some(i)
            })
            .collect();
        for rec in copied_records {
            if rec.copied_role.tags().contains(&tag)
                && !feared_seats.contains(&rec.copycat_seat)
                && !seats.contains(&rec.copycat_seat)
            {
                seats.push(rec.copycat_seat);
            }
        }
        seats
    } else {
        let mut seats: Vec<usize> = roster
            .initial_holders(role)
            .into_iter()
            .filter(|s| !feared_seats.contains(s))
            .collect();
        for rec in copied_records {
            if rec.copied_role.id() == role.id()
                && rec.copied_role.copy_behavior() == CopyBehavior::JoinGroupWake
                && !feared_seats.contains(&rec.copycat_seat)
                && !seats.contains(&rec.copycat_seat)
            {
                seats.push(rec.copycat_seat);
            }
        }
        seats
    };
    active_seats.sort_unstable();
    active_seats.dedup();
    active_seats
}

fn finalize_dusk_boundary(
    roster: &Roster,
    copied_records: &[CopiedRecord],
    board: &mut BoardCards,
    feared_seats: &mut HashSet<usize>,
    all_chronicle: &mut Vec<Text>,
) {
    let empty_fear: HashSet<usize> = HashSet::new();
    let dusk_tail = run_tail_copycats(roster, copied_records, &empty_fear, TierPhase::Dusk, board);
    all_chronicle.extend(dusk_tail);

    let mut feared_vec: Vec<usize> = (0..roster.len())
        .filter(|&s| board.marks[s] == Mark::Fear)
        .collect();
    feared_vec.sort_unstable();
    *feared_seats = feared_vec.iter().copied().collect();

    for &s in &feared_vec {
        let _ = roster.seats[s].user.say(irc_format!(
            "😱 You received the {} during Dusk and are too terrified to wake up tonight!",
            Mark::Fear.styled(),
        ));
        let wakes_in_night = roster.seats[s]
            .initial_role
            .wake()
            .is_some_and(|w| w.phase == WakePhase::Night);
        if wakes_in_night || copied_records.iter().any(|r| r.copycat_seat == s) {
            all_chronicle.push(irc_format!(
                "😱 {} was paralyzed by the {} and could not wake up during Night!",
                roster.seats[s].user.nick().bold(),
                Mark::Fear.styled(),
            ));
        }
    }
}

fn run_tail_copycats(
    roster: &Roster,
    copied_records: &[CopiedRecord],
    feared_seats: &HashSet<usize>,
    phase: TierPhase,
    board: &mut BoardCards,
) -> Vec<Text> {
    let mut entries = Vec::new();
    for rec in copied_records {
        let Some(wake) = rec.copied_role.wake() else {
            continue;
        };
        let is_night_role = wake.phase == WakePhase::Night;
        let matches_phase = match phase {
            TierPhase::Dusk => !is_night_role,
            TierPhase::Night => is_night_role,
        };
        if matches_phase
            && rec.copied_role.copy_behavior() == CopyBehavior::WakeAfterAllMutations
            && !feared_seats.contains(&rec.copycat_seat)
        {
            if let WakeKind::Passive(wake_fn) = wake.kind {
                let base_name = roster.seats[rec.copycat_seat].initial_role.name();
                let label = format!("{base_name}-{}", rec.copied_role.name());
                let mut rctx = ResolveCtx {
                    roster,
                    chronicle: &mut entries,
                };
                wake_fn(rec.copycat_seat, &label, board, &mut rctx);
            }
        }
    }
    entries
}

#[cfg(test)]
#[path = "night_test.rs"]
mod test;

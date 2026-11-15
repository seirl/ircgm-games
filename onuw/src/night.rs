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
pub use crate::role::Phase as TierPhase;
use crate::role::{
    BoxFuture, GroupEffect, GroupPromptCtx, PassiveWakeFn, Phase, PromptCtx, ResolveCtx, Role,
    RoleTag, SeatEffect, WakeKind,
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

#[derive(Clone, Debug)]
pub enum TierKind {
    /// Standard concurrent tier of Dusk or Night roles (resolved in ascending `wake.order`).
    Roles(Vec<&'static dyn Role>),
    /// Sequential per-copy tier when `deck` has multiple copies of a `reads_live_board && mutates_board`
    /// role (`!add 2x witch`, `!add 2x pi`), so later copies see earlier copies' mutations.
    RoleSlot {
        role: &'static dyn Role,
        slot_idx: usize,
        total_slots: usize,
    },
    /// Fixed-clock window after `Doppelgänger` for immediate-copy roles (`Seer`, `Robber`, `Witch`, etc.),
    /// present iff `deck` contains at least one `doppelganger_acts_immediately` role.
    DoppelgangerImmediate(&'static dyn Role),
}

#[derive(Clone, Debug)]
pub struct ScheduledTier {
    pub phase: Phase,
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
            TierKind::RoleSlot {
                role,
                slot_idx,
                total_slots,
            } => format!("{} (#{}/{total_slots})", role.name(), slot_idx + 1),
            TierKind::DoppelgangerImmediate(r) => format!("{} (copied action)", r.name()),
        }
    }

    pub fn styled_callout(&self) -> Text {
        match &self.kind {
            TierKind::Roles(roles) => {
                let parts: Vec<Text> = roles.iter().map(|r| r.styled_name()).collect();
                Text::join(&parts, ", ".grey())
            }
            TierKind::RoleSlot {
                role,
                slot_idx,
                total_slots,
            } => irc_format!(
                "{} {}",
                role.styled_name(),
                format!("(#{}/{total_slots})", slot_idx + 1).grey()
            ),
            TierKind::DoppelgangerImmediate(r) => {
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

    let has_dusk = waking
        .iter()
        .any(|r| r.wake().is_some_and(|w| w.phase == Phase::Dusk));
    let deck_has_immediate_copy = deck.iter().any(|r| r.doppelganger_acts_immediately());

    let effective_order = |r: &'static dyn Role| -> u16 {
        let w = r.wake().expect("filtered to waking roles");
        if w.is_doppelganger {
            if has_dusk { 10 } else { 105 }
        } else {
            w.order
        }
    };
    let effective_phase = |r: &'static dyn Role| -> Phase {
        if has_dusk && effective_order(r) < 100 {
            Phase::Dusk
        } else {
            Phase::Night
        }
    };

    waking.sort_by_key(|&r| (effective_phase(r), effective_order(r), r.id()));

    let mut raw_tiers: Vec<(Phase, TierKind, Duration)> = Vec::new();
    let mut cur: Vec<&'static dyn Role> = Vec::new();
    let mut cur_phase: Option<Phase> = None;
    let mut cur_has_mutator = false;
    let mut cur_has_shield_mutator = false;

    let flush_cur = |cur: &mut Vec<&'static dyn Role>,
                     cur_phase: Option<Phase>,
                     cur_has_mutator: &mut bool,
                     cur_has_shield_mutator: &mut bool,
                     out: &mut Vec<(Phase, TierKind, Duration)>| {
        if !cur.is_empty() {
            out.push((
                cur_phase.expect("cur non-empty implies cur_phase set"),
                TierKind::Roles(std::mem::take(cur)),
                timings.tier_budget,
            ));
            *cur_has_mutator = false;
            *cur_has_shield_mutator = false;
        }
    };

    for role in waking {
        let phase = effective_phase(role);
        let wake = role.wake().expect("filtered to waking roles");

        if cur_phase != Some(phase) {
            flush_cur(
                &mut cur,
                cur_phase,
                &mut cur_has_mutator,
                &mut cur_has_shield_mutator,
                &mut raw_tiers,
            );
            cur_phase = Some(phase);
        }

        // Dedicated short-budget card-selection tiers (`Copycat`, `Doppelgänger`):
        if wake.short_budget {
            flush_cur(
                &mut cur,
                cur_phase,
                &mut cur_has_mutator,
                &mut cur_has_shield_mutator,
                &mut raw_tiers,
            );
            raw_tiers.push((phase, TierKind::Roles(vec![role]), timings.copycat_pick_budget));
            if wake.is_doppelganger && deck_has_immediate_copy {
                raw_tiers.push((
                    phase,
                    TierKind::DoppelgangerImmediate(role),
                    timings.tier_budget,
                ));
            }
            continue;
        }

        let count_in_deck = deck.iter().filter(|&&r| r.id() == role.id()).count();
        if count_in_deck > 1
            && wake.needs_dm_prompt()
            && wake.reads_live_board
            && wake.mutates_board
        {
            flush_cur(
                &mut cur,
                cur_phase,
                &mut cur_has_mutator,
                &mut cur_has_shield_mutator,
                &mut raw_tiers,
            );
            for slot_idx in 0..count_in_deck {
                raw_tiers.push((
                    phase,
                    TierKind::RoleSlot {
                        role,
                        slot_idx,
                        total_slots: count_in_deck,
                    },
                    timings.tier_budget,
                ));
            }
            continue;
        }

        let shares_pack_with_tier = cur.iter().any(|prev| match prev.wake().map(|w| w.kind) {
            Some(WakeKind::Group {
                pack: Some(pack_tag),
                ..
            }) => role.tags().contains(&pack_tag),
            _ => false,
        });
        let must_split = !cur.is_empty()
            && wake.needs_dm_prompt()
            && (cur_has_shield_mutator
                || shares_pack_with_tier
                || (wake.reads_live_board && cur_has_mutator));

        if must_split {
            flush_cur(
                &mut cur,
                cur_phase,
                &mut cur_has_mutator,
                &mut cur_has_shield_mutator,
                &mut raw_tiers,
            );
        }
        if wake.mutates_board {
            cur_has_mutator = true;
        }
        if wake.mutates_shields {
            cur_has_shield_mutator = true;
        }
        cur.push(role);
    }

    flush_cur(
        &mut cur,
        cur_phase,
        &mut cur_has_mutator,
        &mut cur_has_shield_mutator,
        &mut raw_tiers,
    );

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
        label: String,
        wake_fn: PassiveWakeFn,
    },
    Solo {
        label: String,
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
    let mut waking_roles: Vec<&'static dyn Role> =
        roster.seats.iter().map(|s| s.initial_role).collect();
    let mut waking_labels: Vec<String> = roster
        .seats
        .iter()
        .map(|s| s.initial_role.name().to_owned())
        .collect();
    let mut doppelganger_seats: HashSet<usize> = HashSet::new();
    let mut acted_immediately: HashSet<usize> = HashSet::new();

    for (seat, info) in roster.seats.iter().enumerate() {
        let mut rctx = ResolveCtx {
            roster,
            chronicle: &mut all_chronicle,
            waking_roles: &mut waking_roles,
            waking_labels: &mut waking_labels,
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
    let mut current_phase: Option<Phase> = None;
    let mut feared_seats: HashSet<usize> = HashSet::new();
    let mut dusk_finalized = false;

    for (tier_idx, tier) in schedule.iter().enumerate() {
        if current_phase == Some(Phase::Dusk) && tier.phase == Phase::Night && !dusk_finalized {
            finalize_dusk_boundary(
                roster,
                &waking_roles,
                &acted_immediately,
                &board,
                &mut feared_seats,
                &mut all_chronicle,
            );
            dusk_finalized = true;
        }

        if current_phase != Some(tier.phase) {
            match tier.phase {
                Phase::Dusk => {
                    room.say("🌆 Dusk falls! Dusk roles check your DMs...".bold().orange())?;
                }
                Phase::Night => {
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
            TierKind::DoppelgangerImmediate(_) => {
                let mut immediate_seats: Vec<usize> = doppelganger_seats
                    .iter()
                    .copied()
                    .filter(|&s| {
                        !feared_seats.contains(&s) && waking_roles[s].doppelganger_acts_immediately()
                    })
                    .collect();
                immediate_seats.sort_unstable();

                let board_snap = &board;
                let shielded_snap = &board.shielded;
                let waking_snap = waking_roles.as_slice();
                let cfg_ref = &tier_cfg;
                let futs = immediate_seats.iter().map(|&seat| async move {
                    let copied_role = waking_snap[seat];
                    let wake = copied_role.wake().expect("immediate copied role wakes");
                    match wake.kind {
                        WakeKind::Passive(wake_fn) => {
                            Ok::<_, Error>((seat, CopiedImmediateOutcome::Passive(wake_fn)))
                        }
                        WakeKind::Solo(prompt_fn) => {
                            let dm = roster.seats[seat].user.exclusive_available().await?;
                            let pctx = PromptCtx {
                                seat,
                                dm,
                                roster,
                                ask_cfg: cfg_ref,
                                deadline: tier_deadline,
                                shielded: shielded_snap,
                                waking_roles: waking_snap,
                                live_board: wake.reads_live_board.then_some(board_snap),
                            };
                            let effect = prompt_fn(seat, pctx).await?;
                            Ok((seat, CopiedImmediateOutcome::Solo(effect)))
                        }
                        WakeKind::Group { .. } => {
                            unreachable!("doppelganger_acts_immediately only matches Solo/Passive")
                        }
                    }
                });

                let outcomes = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for (seat, outcome) in outcomes {
                    acted_immediately.insert(seat);
                    let label = waking_labels[seat].clone();
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                        waking_roles: &mut waking_roles,
                        waking_labels: &mut waking_labels,
                    };
                    match outcome {
                        CopiedImmediateOutcome::Passive(wake_fn) => {
                            wake_fn(seat, &label, &mut board, &mut rctx);
                        }
                        CopiedImmediateOutcome::Solo(effect) => {
                            effect.apply(&label, &mut board, &mut rctx);
                        }
                    }
                }
            }

            TierKind::RoleSlot {
                role,
                slot_idx,
                total_slots,
            } => {
                let wake = role.wake().expect("scheduled role has Wake metadata");
                let WakeKind::Solo(prompt_fn) = wake.kind else {
                    unreachable!("RoleSlot only applies to interactive Solo roles");
                };

                let all_actors = collect_solo_or_passive_actors(
                    roster,
                    *role,
                    &waking_roles,
                    &waking_labels,
                    &acted_immediately,
                    &feared_seats,
                );
                let slot_actors: &[(usize, String)] = if *slot_idx + 1 < *total_slots {
                    all_actors.get(*slot_idx..=*slot_idx).unwrap_or(&[])
                } else {
                    all_actors.get(*slot_idx..).unwrap_or(&[])
                };

                let board_snap = &board;
                let shielded_snap = &board.shielded;
                let waking_snap = waking_roles.as_slice();
                let cfg_ref = &tier_cfg;

                let futs = slot_actors.iter().map(|(seat, label)| {
                    let seat = *seat;
                    let label = label.clone();
                    async move {
                        let dm = roster.seats[seat].user.exclusive_available().await?;
                        let pctx = PromptCtx {
                            seat,
                            dm,
                            roster,
                            ask_cfg: cfg_ref,
                            deadline: tier_deadline,
                            shielded: shielded_snap,
                            waking_roles: waking_snap,
                            live_board: wake.reads_live_board.then_some(board_snap),
                        };
                        let effect = prompt_fn(seat, pctx).await?;
                        Ok::<_, Error>(RoleTierResolution::Solo { label, effect })
                    }
                });

                let resolutions = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for res in resolutions {
                    if let RoleTierResolution::Solo { label, effect } = res {
                        let mut rctx = ResolveCtx {
                            roster,
                            chronicle: &mut all_chronicle,
                            waking_roles: &mut waking_roles,
                            waking_labels: &mut waking_labels,
                        };
                        effect.apply(&label, &mut board, &mut rctx);
                    }
                }
            }

            TierKind::Roles(roles) => {
                if roles.iter().any(|r| r.is_doppelganger()) {
                    doppelganger_seats = (0..roster.len())
                        .filter(|&s| waking_roles[s].is_doppelganger() && !feared_seats.contains(&s))
                        .collect();
                }

                let board_snap = &board;
                let shielded_snap = &board.shielded;
                let waking_snap = waking_roles.as_slice();
                let feared_snap = &feared_seats;
                let cfg_ref = &tier_cfg;

                let mut futs: Vec<BoxFuture<'_, Result<RoleTierResolution>>> = Vec::new();

                for &role in roles {
                    let wake = role.wake().expect("scheduled role has Wake metadata");
                    match wake.kind {
                        WakeKind::Group { pack, prompt } => {
                            let active_seats = compute_group_active_seats(
                                role,
                                pack,
                                waking_snap,
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
                                    waking_roles: waking_snap,
                                    shielded: shielded_snap,
                                    feared_seats: feared_snap,
                                    live_board: wake.reads_live_board.then_some(board_snap),
                                };
                                let effect = prompt(active_seats, gctx).await?;
                                Ok(RoleTierResolution::Group(effect))
                            }));
                        }
                        WakeKind::Passive(wake_fn) => {
                            for (seat, label) in collect_solo_or_passive_actors(
                                roster,
                                role,
                                waking_snap,
                                &waking_labels,
                                &acted_immediately,
                                feared_snap,
                            ) {
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
                            for (seat, label) in collect_solo_or_passive_actors(
                                roster,
                                role,
                                waking_snap,
                                &waking_labels,
                                &acted_immediately,
                                feared_snap,
                            ) {
                                futs.push(Box::pin(async move {
                                    let dm = roster.seats[seat].user.exclusive_available().await?;
                                    let pctx = PromptCtx {
                                        seat,
                                        dm,
                                        roster,
                                        ask_cfg: cfg_ref,
                                        deadline: tier_deadline,
                                        shielded: shielded_snap,
                                        waking_roles: waking_snap,
                                        live_board: wake.reads_live_board.then_some(board_snap),
                                    };
                                    let effect = prompt_fn(seat, pctx).await?;
                                    Ok(RoleTierResolution::Solo { label, effect })
                                }));
                            }
                        }
                    }
                }

                let resolutions = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for res in resolutions {
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                        waking_roles: &mut waking_roles,
                        waking_labels: &mut waking_labels,
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
                            wake_fn(seat, &label, &mut board, &mut rctx);
                        }
                        RoleTierResolution::Solo { label, effect } => {
                            effect.apply(&label, &mut board, &mut rctx);
                        }
                    }
                }
            }
        }
    }

    if current_phase == Some(Phase::Dusk) && !dusk_finalized {
        finalize_dusk_boundary(
            roster,
            &waking_roles,
            &acted_immediately,
            &board,
            &mut feared_seats,
            &mut all_chronicle,
        );
    }

    Ok((board, all_chronicle))
}

/// Returns `(seat, actor_label)` in resolution order: initial unfeared holders of `role` first,
/// followed by unfeared copiers (`Copycat` / `Doppelgänger` `X-A` sub-wakes) who haven't already acted.
fn collect_solo_or_passive_actors(
    roster: &Roster,
    role: &'static dyn Role,
    waking_roles: &[&'static dyn Role],
    waking_labels: &[String],
    acted_immediately: &HashSet<usize>,
    feared_seats: &HashSet<usize>,
) -> Vec<(usize, String)> {
    let target_id = role.id();
    let is_eligible = |s: usize| {
        waking_roles[s].id() == target_id
            && !acted_immediately.contains(&s)
            && !feared_seats.contains(&s)
    };

    let mut actors: Vec<(usize, String)> = Vec::new();
    for s in 0..roster.len() {
        if is_eligible(s) && roster.seats[s].initial_role.id() == target_id {
            actors.push((s, waking_labels[s].clone()));
        }
    }
    for s in 0..roster.len() {
        if is_eligible(s) && roster.seats[s].initial_role.id() != target_id {
            actors.push((s, waking_labels[s].clone()));
        }
    }
    actors
}

fn compute_group_active_seats(
    role: &'static dyn Role,
    tag_opt: Option<RoleTag>,
    waking_roles: &[&'static dyn Role],
    feared_seats: &HashSet<usize>,
) -> Vec<usize> {
    let target_id = role.id();
    waking_roles
        .iter()
        .enumerate()
        .filter_map(|(s, r)| {
            if feared_seats.contains(&s) {
                return None;
            }
            let matches = match tag_opt {
                Some(tag) => r.tags().contains(&tag),
                None => r.id() == target_id,
            };
            matches.then_some(s)
        })
        .collect()
}

fn finalize_dusk_boundary(
    roster: &Roster,
    waking_roles: &[&'static dyn Role],
    acted_immediately: &HashSet<usize>,
    board: &BoardCards,
    feared_seats: &mut HashSet<usize>,
    all_chronicle: &mut Vec<Text>,
) {
    let marks_in_play =
        roster.deck_has_marks() || board.marks.iter().any(|&m| m != Mark::Clarity);

    let mut feared_vec: Vec<usize> = (0..roster.len())
        .filter(|&s| board.marks[s] == Mark::Fear)
        .collect();
    feared_vec.sort_unstable();
    *feared_seats = feared_vec.iter().copied().collect();

    if marks_in_play {
        for s in 0..roster.len() {
            let mark = board.marks[s];
            if mark == Mark::Fear {
                let _ = roster.seats[s].user.say(irc_format!(
                    "😱 Dusk ends — your Mark is {}! You received the {} during Dusk and are too terrified to wake up tonight!",
                    Mark::Fear.styled(),
                    Mark::Fear.styled(),
                ));
            } else {
                let _ = roster.seats[s].user.say(irc_format!(
                    "🔖 Dusk ends — you check your Mark: {}.",
                    mark.styled(),
                ));
            }
        }
    }

    for &s in &feared_vec {
        if !marks_in_play {
            let _ = roster.seats[s].user.say(irc_format!(
                "😱 You received the {} during Dusk and are too terrified to wake up tonight!",
                Mark::Fear.styled(),
            ));
        }
        let r = waking_roles[s];
        let wakes_in_night = !acted_immediately.contains(&s)
            && (r.wake()
                .is_some_and(|w| w.phase == Phase::Night && w.order >= 100)
                || r.tags().iter().any(|t| {
                    matches!(
                        t,
                        RoleTag::WolfPack | RoleTag::AlienPack | RoleTag::VillainPack
                    )
                }));
        if wakes_in_night {
            all_chronicle.push(irc_format!(
                "😱 {} was paralyzed by the {} and could not wake up during Night!",
                roster.seats[s].user.nick().bold(),
                Mark::Fear.styled(),
            ));
        }
    }

    // `00: Lovers` — players holding `Mark::Love` wake at the end of Dusk to recognize each other:
    let lovers: Vec<usize> = (0..roster.len())
        .filter(|&s| board.marks[s] == Mark::Love)
        .collect();
    if lovers.len() >= 2 {
        for &s in &lovers {
            let partners: Vec<usize> = lovers.iter().copied().filter(|&o| o != s).collect();
            let _ = roster.seats[s].user.say(irc_format!(
                "💘 Lovers wake: Your Lover is {} — keep each other alive!",
                roster.format_indices(&partners).bold().magenta(),
            ));
        }
        all_chronicle.push(irc_format!(
            "💘 Lovers ({}) woke at the end of Dusk and recognized each other.",
            roster.format_indices(&lovers).bold().magenta(),
        ));
    }
}

#[cfg(test)]
#[path = "night_test.rs"]
mod test;

//! Anti-Timing-Attack **Deck-Derived Fixed-Clock Tier Scheduler** for Dusk & Night.
//!
//! - [`build_deck_schedule`] derives the complete sequence of [`ScheduledTier`]s and their exact
//!   wall-clock offsets **purely from the public `deck` multiset** (never from dealt seats, center
//!   cards, copycat targets, `Mark::Fear`, or player response speed).
//! - Every step in a tier starts at `origin + tier.start_offset`, and [`run_night_phase`]
//!   unconditionally waits until `origin + tier.end_offset` (`tokio::time::sleep_until`) before
//!   committing mutations, sending passive/third-party DMs, or advancing to the next tier.

use std::time::Duration;

use futures::future::try_join_all;
use ircgm::prelude::*;
use tokio::time::{sleep_until, Instant};

use crate::board::{BoardAccess, BoardCards, BoardMut, PromptBoard, Roster, SeatSet};
use crate::marks::Mark;
pub use crate::role::Phase as TierPhase;
use crate::role::{
    BoxFuture, CopierKind, CopyTiming, GroupEffect, GroupPromptCtx, GroupPromptFn, NightWakers,
    PassiveWakeFn, Phase, PromptCtx, ResolveCtx, Role, SeatEffect, SoloPromptFn, StepKind,
    WakeStep,
};
use crate::roles::{dusk_immediate_rank, is_night_step, night_immediate_rank, step_exec_rank};

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
    /// Standard concurrent tier of Dusk or Night steps (resolved in ascending `step.order`).
    Steps(Vec<&'static WakeStep>),
    /// Sequential per-copy tier when `deck` has multiple copies of a step whose prompt reads
    /// conflict with its own writes (`!add 2x witch`, `!add 2x curator`, `!add 2x pi`).
    StepSlot {
        step: &'static WakeStep,
        slot_idx: usize,
        total_slots: usize,
    },
    /// Fixed-clock window right after `Doppelgänger` for `CopyTiming::AfterPlayerCopier` group steps (`Oracle`),
    /// present iff `deck` contains both a player-copier and an `AfterPlayerCopier` step.
    AfterPlayerCopier {
        copier: &'static WakeStep,
        step: &'static WakeStep,
    },
    /// Fixed-clock window after `Doppelgänger` for `CopyTiming::Immediate` roles (`Seer`, `Robber`, `Witch`, etc.),
    /// present iff `deck` contains at least one immediate-copy role.
    DoppelgangerImmediate(&'static WakeStep),
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
            TierKind::Steps(steps) => steps
                .iter()
                .map(|st| st.callout_name())
                .collect::<Vec<_>>()
                .join(", "),
            TierKind::StepSlot {
                step,
                slot_idx,
                total_slots,
            } => format!("{} (#{}/{total_slots})", step.callout_name(), slot_idx + 1),
            TierKind::AfterPlayerCopier { copier, step } => {
                format!("{}-{}", copier.callout_name(), step.callout_name())
            }
            TierKind::DoppelgangerImmediate(st) => {
                format!("{} (copied action)", st.callout_name())
            }
        }
    }

    pub fn styled_callout(&self) -> Text {
        match &self.kind {
            TierKind::Steps(steps) => {
                let parts: Vec<Text> = steps.iter().map(|st| st.styled_callout()).collect();
                Text::join(&parts, ", ".grey())
            }
            TierKind::StepSlot {
                step,
                slot_idx,
                total_slots,
            } => irc_format!(
                "{} {}",
                step.styled_callout(),
                format!("(#{}/{total_slots})", slot_idx + 1).grey()
            ),
            TierKind::AfterPlayerCopier { copier, step } => irc_format!(
                "{}{}{}",
                copier.styled_callout(),
                "-".grey(),
                step.styled_callout()
            ),
            TierKind::DoppelgangerImmediate(st) => {
                irc_format!("{} {}", st.styled_callout(), "(copied action)".grey())
            }
        }
    }
}

fn step_exec_phase(st: &WakeStep, has_dusk: bool) -> Phase {
    if has_dusk && !is_night_step(st, true) {
        Phase::Dusk
    } else {
        Phase::Night
    }
}

/// Formats the canonical Dusk & Night wake order for `deck` (pure function of the `deck` multiset).
pub fn format_role_order(deck: &[&'static dyn Role]) -> Text {
    format_schedule_order(&build_deck_schedule(deck, &NightTimings::default()))
}

fn format_schedule_order(schedule: &[ScheduledTier]) -> Text {
    if schedule.is_empty() {
        return irc_format!("📋 {}: {}", "Role order".bold(), "No waking roles".grey());
    }
    let has_dusk = schedule.iter().any(|t| t.phase == Phase::Dusk);
    let mut dusk = Vec::new();
    let mut night = Vec::new();
    for tier in schedule {
        let out = match tier.phase {
            Phase::Dusk => &mut dusk,
            Phase::Night => &mut night,
        };
        match &tier.kind {
            TierKind::Steps(steps) => out.extend(steps.iter().map(|st| st.styled_callout())),
            TierKind::StepSlot {
                step, slot_idx: 0, ..
            } => out.push(step.styled_callout()),
            TierKind::StepSlot { .. } => {}
            TierKind::AfterPlayerCopier { .. } => {}
            TierKind::DoppelgangerImmediate(_) if has_dusk && tier.phase == Phase::Night => {
                out.push(tier.styled_callout());
            }
            TierKind::DoppelgangerImmediate(_) => {}
        }
    }
    let arrow = " → ".grey();
    match (!dusk.is_empty(), !night.is_empty()) {
        (false, _) => irc_format!("📋 {}: {}", "Role order".bold(), Text::join(&night, arrow)),
        (true, false) => irc_format!(
            "📋 {} {} {} {}",
            "Role order".bold(),
            "—".grey(),
            "Dusk:".bold().orange(),
            Text::join(&dusk, arrow),
        ),
        (true, true) => irc_format!(
            "📋 {} {} {} {}{}{} {}",
            "Role order".bold(),
            "—".grey(),
            "Dusk:".bold().orange(),
            Text::join(&dusk, arrow.clone()),
            " · ".grey(),
            "Night:".bold().light_blue(),
            Text::join(&night, arrow),
        ),
    }
}

#[derive(Clone, Copy)]
enum SchedEntry {
    Step(&'static WakeStep),
    AfterPlayerCopier {
        copier: &'static WakeStep,
        step: &'static WakeStep,
    },
    DgImmediate(&'static WakeStep),
}

/// Pure function of the public `deck` multiset and `timings` — any permutation of the same
/// `(N + 3)` cards produces the exact same tier list and wall-clock offsets.
fn collect_deck_steps(deck: &[&'static dyn Role]) -> Vec<&'static WakeStep> {
    let mut steps: Vec<&'static WakeStep> = Vec::new();
    for &r in deck {
        for st in r.steps() {
            if !steps.iter().any(|existing| existing.id() == st.id()) {
                steps.push(st);
            }
        }
    }
    steps
}

pub fn build_deck_schedule(
    deck: &[&'static dyn Role],
    timings: &NightTimings,
) -> Vec<ScheduledTier> {
    let steps = collect_deck_steps(deck);

    let has_dusk = steps.iter().any(|st| st.phase == Phase::Dusk);
    let player_count = deck.len().saturating_sub(3).max(1);
    let player_copier_count = deck.iter().filter(|&&r| r.is_player_copier()).count();

    let mut entries: Vec<(Phase, usize, u8, &'static str, SchedEntry)> = steps
        .iter()
        .map(|&st| {
            (
                step_exec_phase(st, has_dusk),
                step_exec_rank(st, has_dusk),
                0,
                st.id(),
                SchedEntry::Step(st),
            )
        })
        .collect();

    if let Some(dg) = steps
        .iter()
        .copied()
        .find(|st| st.copier == Some(CopierKind::Player))
    {
        let dg_phase = step_exec_phase(dg, has_dusk);
        let dg_order = step_exec_rank(dg, has_dusk);
        for &st in &steps {
            if st.copy_timing == Some(CopyTiming::AfterPlayerCopier) {
                entries.push((
                    dg_phase,
                    dg_order,
                    1,
                    st.id(),
                    SchedEntry::AfterPlayerCopier {
                        copier: dg,
                        step: st,
                    },
                ));
            }
        }
        for (imm_phase, imm_order) in [
            (Phase::Dusk, dusk_immediate_rank()),
            (Phase::Night, night_immediate_rank()),
        ] {
            let needed = deck.iter().any(|r| {
                r.immediate_copy_step()
                    .is_some_and(|st| step_exec_phase(st, has_dusk) == imm_phase)
            });
            if needed {
                entries.push((imm_phase, imm_order, 2, dg.id(), SchedEntry::DgImmediate(dg)));
            }
        }
    }

    entries.sort_by_key(|&(phase, ord, sub, id, _)| (phase, ord, sub, id));

    let mut raw_tiers: Vec<(Phase, TierKind, Duration)> = Vec::new();
    let mut cur: Vec<&'static WakeStep> = Vec::new();
    let mut cur_phase: Option<Phase> = None;
    let mut cur_writes: u16 = 0;

    let flush_cur = |cur: &mut Vec<&'static WakeStep>,
                     cur_phase: Option<Phase>,
                     cur_writes: &mut u16,
                     out: &mut Vec<(Phase, TierKind, Duration)>| {
        if !cur.is_empty() {
            out.push((
                cur_phase.expect("cur non-empty implies cur_phase set"),
                TierKind::Steps(std::mem::take(cur)),
                timings.tier_budget,
            ));
            *cur_writes = 0;
        }
    };

    for (phase, _, _, _, entry) in entries {
        if cur_phase != Some(phase) {
            flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
            cur_phase = Some(phase);
        }

        let step = match entry {
            SchedEntry::AfterPlayerCopier { copier, step } => {
                flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
                raw_tiers.push((
                    phase,
                    TierKind::AfterPlayerCopier { copier, step },
                    timings.tier_budget,
                ));
                continue;
            }
            SchedEntry::DgImmediate(dg) => {
                flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
                raw_tiers.push((
                    phase,
                    TierKind::DoppelgangerImmediate(dg),
                    timings.tier_budget,
                ));
                continue;
            }
            SchedEntry::Step(st) => st,
        };

        // Dedicated short-budget card-selection tiers (`Copycat`, `Doppelgänger`):
        if step.short_budget {
            flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
            raw_tiers.push((
                phase,
                TierKind::Steps(vec![step]),
                timings.copycat_pick_budget,
            ));
            continue;
        }

        if matches!(step.kind, StepKind::Solo(_)) {
            let direct = deck.iter().filter(|&&r| r.has_step(step)).count();
            let cc_extra = deck
                .iter()
                .filter(|&&r| {
                    r.steps()
                        .iter()
                        .find(|s| s.copier == Some(CopierKind::Center))
                        .is_some_and(|cc_st| {
                            step_exec_rank(step, has_dusk) > step_exec_rank(cc_st, has_dusk)
                        })
                })
                .count();
            let holders_before_dg = direct.max(direct.saturating_sub(1) + cc_extra);
            let dg_extra = if step.copy_timing == Some(CopyTiming::OnRoleTurn) {
                player_copier_count
            } else {
                0
            };
            let max_actors = (holders_before_dg + dg_extra).min(player_count);
            let self_reads = step.effective_prompt_reads() & !BoardAccess::PUBLIC_ANNOUNCE;
            if max_actors > 1 && (self_reads & step.resolve_writes.0) != 0 {
                flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
                for slot_idx in 0..max_actors {
                    raw_tiers.push((
                        phase,
                        TierKind::StepSlot {
                            step,
                            slot_idx,
                            total_slots: max_actors,
                        },
                        timings.tier_budget,
                    ));
                }
                continue;
            }
        }

        let shares_pack_with_tier = match step.owner_night_faction() {
            Some(fac) => cur.iter().any(|prev| prev.pack_faction() == Some(fac)),
            None => false,
        };
        let has_data_hazard = (cur_writes & step.effective_prompt_reads()) != 0;
        let must_split = !cur.is_empty()
            && step.has_tier_start()
            && (shares_pack_with_tier || has_data_hazard);

        if must_split {
            flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
        }
        cur_writes |= step.resolve_writes.0;
        cur.push(step);
    }

    flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);

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

enum StepTierResolution {
    Group {
        step: &'static WakeStep,
        effect: GroupEffect,
    },
    Passive {
        step: &'static WakeStep,
        seat: usize,
        wake_fn: PassiveWakeFn,
        mark_acted_immediately: bool,
    },
    Solo {
        step: &'static WakeStep,
        seat: usize,
        effect: SeatEffect,
        mark_acted_immediately: bool,
    },
    TierEpilogue {
        step: &'static WakeStep,
        after_fn: fn(&mut BoardMut<'_>, &mut ResolveCtx<'_>),
    },
}

impl StepTierResolution {
    fn step(&self) -> &'static WakeStep {
        match self {
            Self::Group { step, .. }
            | Self::Passive { step, .. }
            | Self::Solo { step, .. }
            | Self::TierEpilogue { step, .. } => step,
        }
    }

    fn resolve_reads(&self) -> u16 {
        match self {
            Self::Passive { .. } => BoardAccess::ALL_READS,
            Self::Group { step, .. }
            | Self::Solo { step, .. }
            | Self::TierEpilogue { step, .. } => step.effective_prompt_reads(),
        }
    }
}

fn record_step_wakers(
    out: &mut Vec<(String, SeatSet)>,
    step: &WakeStep,
    primary_label: String,
    seats: SeatSet,
    wakers: &NightWakers,
) {
    if seats.is_empty() {
        return;
    }
    if step.copy_timing == Some(CopyTiming::OnRoleTurn) {
        let dg_seats = seats & wakers.player_copied_seats();
        let primary_seats = seats - dg_seats;
        if !primary_seats.is_empty() {
            out.push((primary_label, primary_seats));
        }
        if !dg_seats.is_empty() {
            out.push((format!("Doppelgänger-{}", step.callout_name()), dg_seats));
        }
    } else {
        out.push((primary_label, seats));
    }
}

struct TierPromptSpawner<'a> {
    roster: &'a Roster,
    cfg: &'a AskConfig,
    deadline: Instant,
    wakers: &'a NightWakers,
    board_snap: PromptBoard<'a>,
}

impl<'a> TierPromptSpawner<'a> {
    fn group(
        &self,
        step: &'static WakeStep,
        prompt_fn: GroupPromptFn,
        active_seats: SeatSet,
        player_copy_turn: bool,
    ) -> BoxFuture<'a, Result<StepTierResolution>> {
        let Self {
            roster,
            cfg,
            deadline,
            wakers,
            board_snap,
        } = *self;
        Box::pin(async move {
            let mut gctx = GroupPromptCtx::new(
                step.callout_name(),
                step.effective_prompt_reads(),
                step.eyes_open,
                active_seats,
                roster,
                cfg,
                deadline,
                wakers,
                board_snap,
            );
            if player_copy_turn {
                gctx = gctx.as_player_copy_turn();
            }
            let effect = prompt_fn(active_seats, gctx).await?;
            Ok(StepTierResolution::Group { step, effect })
        })
    }

    fn solo(
        &self,
        step: &'static WakeStep,
        prompt_fn: SoloPromptFn,
        seat: usize,
        immediate_copy: bool,
    ) -> BoxFuture<'a, Result<StepTierResolution>> {
        let Self {
            roster,
            cfg,
            deadline,
            wakers,
            board_snap,
        } = *self;
        Box::pin(async move {
            let dm = roster.seats[seat].user.exclusive_available().await?;
            let mut pctx = PromptCtx::new(
                step.callout_name(),
                step.effective_prompt_reads(),
                seat,
                step.eyes_open,
                dm,
                roster,
                cfg,
                deadline,
                wakers,
                board_snap,
            );
            if immediate_copy {
                pctx = pctx.as_immediate_copy();
            }
            let effect = prompt_fn(seat, pctx).await?;
            Ok(StepTierResolution::Solo {
                step,
                seat,
                effect,
                mark_acted_immediately: immediate_copy,
            })
        })
    }
}

pub async fn run_night_phase(
    room: &Room,
    roster: &Roster,
    mut board: BoardCards,
    timings: &NightTimings,
) -> Result<(BoardCards, Vec<Text>)> {
    let mut all_chronicle: Vec<Text> = Vec::new();
    let mut wakers = NightWakers::new(roster);

    let schedule = build_deck_schedule(&roster.deck, timings);
    room.say(format_schedule_order(&schedule))?;
    if schedule.is_empty() {
        room.say("🌙 Night falls over a peaceful village...".bold().light_blue())?;
        return Ok((board, all_chronicle));
    }

    let has_dusk = schedule.iter().any(|t| t.phase == Phase::Dusk);
    let origin = Instant::now();
    let total_tiers = schedule.len();
    let mut current_phase: Option<Phase> = None;
    let mut dusk_finalized = false;

    for (tier_idx, tier) in schedule.iter().enumerate() {
        if current_phase == Some(Phase::Dusk) && tier.phase == Phase::Night && !dusk_finalized {
            Mark::finalize_dusk_boundary(roster, &board, &mut wakers, &mut all_chronicle);
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
            TierKind::StepSlot {
                step, slot_idx: 0, ..
            } => {
                if let Some(hooks) = step.tier_hooks {
                    let mut bm = BoardMut::new(
                        &mut board,
                        step.callout_name(),
                        step.effective_prompt_reads(),
                        step.resolve_writes.0,
                    );
                    (hooks.before_first_slot)(&mut bm, roster);
                }
            }
            TierKind::Steps(steps) => {
                for &step in steps {
                    if let Some(hooks) = step.tier_hooks {
                        let mut bm = BoardMut::new(
                            &mut board,
                            step.callout_name(),
                            step.effective_prompt_reads(),
                            step.resolve_writes.0,
                        );
                        (hooks.before_first_slot)(&mut bm, roster);
                    }
                }
            }
            _ => {}
        }

        let (resolutions, tier_step_wakers) = {
            let spawner = TierPromptSpawner {
                roster,
                cfg: &tier_cfg,
                deadline: tier_deadline,
                wakers: &wakers,
                board_snap: PromptBoard::new(&board),
            };
            let mut futs: Vec<BoxFuture<'_, Result<StepTierResolution>>> = Vec::new();
            let mut tier_step_wakers: Vec<(String, SeatSet)> = Vec::new();

            match &tier.kind {
                TierKind::AfterPlayerCopier { step, .. } => {
                    let StepKind::Group(prompt_fn) = step.kind else {
                        unreachable!("only Group steps support CopyTiming::AfterPlayerCopier");
                    };
                    let active_seats =
                        compute_group_active_seats(step, &wakers) & wakers.player_copied_seats();
                    if step.eyes_open && !active_seats.is_empty() {
                        tier_step_wakers.push((tier.callout_label(), active_seats));
                    }
                    if !active_seats.is_empty() || step.run_when_unheld {
                        futs.push(spawner.group(step, prompt_fn, active_seats, true));
                    }
                }

                TierKind::DoppelgangerImmediate(_) => {
                    let immediate_seats: SeatSet = (wakers.player_copied_seats()
                        - wakers.feared
                        - wakers.acted_immediately)
                        .into_iter()
                        .filter(|&s| {
                            wakers
                                .role(s)
                                .immediate_copy_step()
                                .is_some_and(|st| step_exec_phase(st, has_dusk) == tier.phase)
                        })
                        .collect();
                    let open_eye_immediate: SeatSet = immediate_seats
                        .into_iter()
                        .filter(|&s| {
                            wakers
                                .role(s)
                                .immediate_copy_step()
                                .is_some_and(|st| st.eyes_open)
                        })
                        .collect();
                    if !open_eye_immediate.is_empty() {
                        tier_step_wakers.push((tier.callout_label(), open_eye_immediate));
                    }
                    for seat in immediate_seats {
                        let step = wakers
                            .role(seat)
                            .immediate_copy_step()
                            .expect("immediate copied role has immediate step");
                        match step.kind {
                            StepKind::Solo(prompt_fn) => {
                                futs.push(spawner.solo(step, prompt_fn, seat, true));
                            }
                            StepKind::Passive(wake_fn) => {
                                futs.push(Box::pin(async move {
                                    Ok(StepTierResolution::Passive {
                                        step,
                                        seat,
                                        wake_fn,
                                        mark_acted_immediately: true,
                                    })
                                }));
                            }
                            StepKind::Group(_) => {
                                unreachable!("Group steps do not support CopyTiming::Immediate");
                            }
                        }
                    }
                }

                TierKind::StepSlot {
                    step,
                    slot_idx,
                    total_slots,
                } => {
                    let StepKind::Solo(prompt_fn) = step.kind else {
                        unreachable!("StepSlot only applies to interactive Solo steps");
                    };
                    let all_actors = collect_solo_or_passive_actors(roster, step, &wakers);
                    let slot_actors: &[usize] = if *slot_idx + 1 < *total_slots {
                        all_actors.get(*slot_idx..=*slot_idx).unwrap_or(&[])
                    } else {
                        all_actors.get(*slot_idx..).unwrap_or(&[])
                    };
                    if step.eyes_open && !slot_actors.is_empty() {
                        record_step_wakers(
                            &mut tier_step_wakers,
                            step,
                            tier.callout_label(),
                            slot_actors.iter().copied().collect(),
                            &wakers,
                        );
                    }
                    for &seat in slot_actors {
                        futs.push(spawner.solo(step, prompt_fn, seat, false));
                    }
                    if *slot_idx + 1 == *total_slots {
                        if let Some(hooks) = step.tier_hooks {
                            let after_fn = hooks.after_last_slot;
                            futs.push(Box::pin(async move {
                                Ok(StepTierResolution::TierEpilogue { step, after_fn })
                            }));
                        }
                    }
                }

                TierKind::Steps(steps) => {
                    for &step in steps {
                        match step.kind {
                            StepKind::Group(prompt_fn) => {
                                let active_seats = compute_group_active_seats(step, &wakers);
                                if step.eyes_open && !active_seats.is_empty() {
                                    record_step_wakers(
                                        &mut tier_step_wakers,
                                        step,
                                        step.callout_name().to_owned(),
                                        active_seats,
                                        &wakers,
                                    );
                                }
                                if active_seats.is_empty() && !step.run_when_unheld {
                                    continue;
                                }
                                futs.push(spawner.group(step, prompt_fn, active_seats, false));
                            }
                            StepKind::Passive(wake_fn) => {
                                let actors = collect_solo_or_passive_actors(roster, step, &wakers);
                                if step.eyes_open && !actors.is_empty() {
                                    record_step_wakers(
                                        &mut tier_step_wakers,
                                        step,
                                        step.callout_name().to_owned(),
                                        actors.iter().copied().collect(),
                                        &wakers,
                                    );
                                }
                                for seat in actors {
                                    futs.push(Box::pin(async move {
                                        Ok(StepTierResolution::Passive {
                                            step,
                                            seat,
                                            wake_fn,
                                            mark_acted_immediately: false,
                                        })
                                    }));
                                }
                                if let Some(hooks) = step.tier_hooks {
                                    let after_fn = hooks.after_last_slot;
                                    futs.push(Box::pin(async move {
                                        Ok(StepTierResolution::TierEpilogue { step, after_fn })
                                    }));
                                }
                            }
                            StepKind::Solo(prompt_fn) => {
                                let actors = collect_solo_or_passive_actors(roster, step, &wakers);
                                if step.eyes_open && !actors.is_empty() {
                                    record_step_wakers(
                                        &mut tier_step_wakers,
                                        step,
                                        step.callout_name().to_owned(),
                                        actors.iter().copied().collect(),
                                        &wakers,
                                    );
                                }
                                for seat in actors {
                                    futs.push(spawner.solo(step, prompt_fn, seat, false));
                                }
                                if let Some(hooks) = step.tier_hooks {
                                    let after_fn = hooks.after_last_slot;
                                    futs.push(Box::pin(async move {
                                        Ok(StepTierResolution::TierEpilogue { step, after_fn })
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            (try_join_all(futs).await?, tier_step_wakers)
        };

        sleep_until(tier_deadline).await;

        if !wakers.wake_log_recipients.is_empty() {
            for (step_name, seats) in tier_step_wakers {
                wakers.wake_log.push((
                    tier.phase,
                    format!("{step_name}: {}", roster.format_seats(seats)),
                ));
            }
        }

        for res in resolutions {
            let step = res.step();
            let table_summary = board.visible_table_summary(roster);
            let mut rctx = ResolveCtx {
                roster,
                chronicle: &mut all_chronicle,
                room,
                wakers: &mut wakers,
                step_name: step.callout_name(),
                writes: step.resolve_writes.0,
            };
            let mut bm = BoardMut::new(
                &mut board,
                step.callout_name(),
                res.resolve_reads(),
                step.resolve_writes.0,
            );
            match res {
                StepTierResolution::Group { effect, .. } => {
                    effect.apply(&mut bm, &mut rctx);
                }
                StepTierResolution::Passive {
                    seat,
                    wake_fn,
                    mark_acted_immediately,
                    ..
                } => {
                    if mark_acted_immediately {
                        rctx.wakers.acted_immediately.insert(seat);
                    }
                    let label = rctx.wakers.actor_label(roster, seat);
                    if step.eyes_open {
                        if let Some(line) = table_summary {
                            rctx.dm(seat, line);
                        }
                    }
                    wake_fn(seat, &label, &mut bm, &mut rctx);
                }
                StepTierResolution::Solo {
                    seat,
                    effect,
                    mark_acted_immediately,
                    ..
                } => {
                    if mark_acted_immediately {
                        rctx.wakers.acted_immediately.insert(seat);
                    }
                    let label = rctx.wakers.actor_label(roster, seat);
                    effect.apply(&label, &mut bm, &mut rctx);
                }
                StepTierResolution::TierEpilogue { after_fn, .. } => {
                    after_fn(&mut bm, &mut rctx);
                }
            }
        }
    }

    if current_phase == Some(Phase::Dusk) && !dusk_finalized {
        Mark::finalize_dusk_boundary(roster, &board, &mut wakers, &mut all_chronicle);
    }

    for observer in wakers.wake_log_recipients {
        let feared_in_night = wakers.feared.contains(observer);
        let visible_entries: Vec<&str> = wakers
            .wake_log
            .iter()
            .filter(|(ph, _)| !feared_in_night || *ph == Phase::Dusk)
            .map(|(_, s)| s.as_str())
            .collect();
        if visible_entries.is_empty() {
            let _ = roster.seats[observer].user.say(
                "🔮🛸 Oracle All-Night Watch: Nobody else opened their eyes tonight!",
            );
        } else {
            let _ = roster.seats[observer].user.say(irc_format!(
                "🔮🛸 Oracle All-Night Watch (who opened their eyes): {}",
                visible_entries.join(" · ").bold(),
            ));
        }
    }

    // Daybreak rulebook: players inspect any Artifact token on their card when everyone wakes up
    // at Daybreak (after all Night roles, including #13-A Mortician, have completed).
    for seat in 0..roster.len() {
        if let Some(art) = board.artifact(seat) {
            let _ = roster.seats[seat].user.say(art.recipient_dm());
        }
    }

    Ok((board, all_chronicle))
}

/// Returns seats in resolution order: initial unfeared holders of `step` first,
/// followed by unfeared copiers (`Copycat` / `Doppelgänger` `X-A` sub-wakes) who haven't already acted.
fn collect_solo_or_passive_actors(
    roster: &Roster,
    step: &'static WakeStep,
    wakers: &NightWakers,
) -> Vec<usize> {
    let eligible = roster.all_seats() - wakers.acted_immediately - wakers.feared;

    let mut actors: Vec<usize> = Vec::new();
    for s in eligible {
        if wakers.role(s).has_step(step) && roster.seats[s].initial_role.has_step(step) {
            actors.push(s);
        }
    }
    // Player-copied (`Doppelgänger`) seats with `CopyTiming::Immediate` execute exclusively in
    // `TierKind::DoppelgangerImmediate` (#1 / #-7), never in an earlier regular tier (#0 Sentinel).
    let copier_eligible = if step.copy_timing == Some(CopyTiming::Immediate) {
        eligible - wakers.player_copied_seats()
    } else {
        eligible
    };
    for s in copier_eligible {
        if wakers.role(s).has_step(step) && !roster.seats[s].initial_role.has_step(step) {
            actors.push(s);
        }
    }
    actors
}

fn compute_group_active_seats(step: &'static WakeStep, wakers: &NightWakers) -> SeatSet {
    wakers
        .roles()
        .iter()
        .enumerate()
        .filter_map(|(s, r)| (!wakers.feared.contains(s) && r.has_step(step)).then_some(s))
        .collect()
}

#[cfg(test)]
#[path = "night_test.rs"]
mod test;

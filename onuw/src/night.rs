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
    BoxFuture, CopierKind, CopyTiming, GroupEffect, GroupPromptCtx, NightWakers, PassiveWakeFn,
    Phase, PromptCtx, ResolveCtx, Role, SeatEffect, StepKind, WakeStep,
};

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
            TierKind::DoppelgangerImmediate(st) => {
                irc_format!("{} {}", st.styled_callout(), "(copied action)".grey())
            }
        }
    }
}

fn step_exec_order(st: &WakeStep, has_dusk: bool) -> u16 {
    if st.copier == Some(CopierKind::Player) && has_dusk {
        10
    } else {
        st.order
    }
}

fn step_exec_phase(st: &WakeStep, has_dusk: bool) -> Phase {
    if has_dusk && step_exec_order(st, has_dusk) < 100 {
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
    DgImmediate(&'static WakeStep),
}

/// Pure function of the public `deck` multiset and `timings` — any permutation of the same
/// `(N + 3)` cards produces the exact same tier list and wall-clock offsets.
pub fn build_deck_schedule(
    deck: &[&'static dyn Role],
    timings: &NightTimings,
) -> Vec<ScheduledTier> {
    let mut steps: Vec<&'static WakeStep> = Vec::new();
    for &r in deck {
        for st in r.steps() {
            if !steps.iter().any(|existing| existing.id() == st.id()) {
                steps.push(st);
            }
        }
    }

    let has_dusk = steps.iter().any(|st| st.phase == Phase::Dusk);
    let player_count = deck.len().saturating_sub(3).max(1);
    let player_copier_count = deck.iter().filter(|&&r| r.is_player_copier()).count();

    let mut entries: Vec<(Phase, u16, u8, &'static str, SchedEntry)> = steps
        .iter()
        .map(|&st| {
            (
                step_exec_phase(st, has_dusk),
                step_exec_order(st, has_dusk),
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
        for (imm_phase, imm_order) in [(Phase::Dusk, 10), (Phase::Night, 105)] {
            let needed = deck.iter().any(|r| {
                r.immediate_copy_step()
                    .is_some_and(|st| step_exec_phase(st, has_dusk) == imm_phase)
            });
            if needed {
                entries.push((imm_phase, imm_order, 1, dg.id(), SchedEntry::DgImmediate(dg)));
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
                    r.center_copier_order()
                        .is_some_and(|cc_ord| step.order > cc_ord)
                })
                .count();
            let holders_before_dg = direct.max(direct.saturating_sub(1) + cc_extra);
            let dg_extra = if step.copy_timing == Some(CopyTiming::OnRoleTurn) {
                player_copier_count
            } else {
                0
            };
            let max_actors = (holders_before_dg + dg_extra).min(player_count);
            let eff_reads = step.effective_prompt_reads();
            if max_actors > 1 && (eff_reads & step.resolve_writes.0) != 0 {
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
            && step.needs_dm_prompt()
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
    },
    Solo {
        step: &'static WakeStep,
        seat: usize,
        effect: SeatEffect,
    },
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
            TierKind::DoppelgangerImmediate(_) => {
                let tier_phase = tier.phase;
                let immediate_seats: SeatSet = (wakers.player_copied_seats()
                    - wakers.feared
                    - wakers.acted_immediately)
                    .into_iter()
                    .filter(|&s| {
                        wakers
                            .role(s)
                            .immediate_copy_step()
                            .is_some_and(|st| step_exec_phase(st, has_dusk) == tier_phase)
                    })
                    .collect();

                let board_snap = PromptBoard::new(&board);
                let wakers_ref = &wakers;
                let cfg_ref = &tier_cfg;
                let futs = immediate_seats.into_iter().map(|seat| async move {
                    let step = wakers_ref
                        .role(seat)
                        .immediate_copy_step()
                        .expect("immediate copied role has immediate step");
                    let StepKind::Solo(prompt_fn) = step.kind else {
                        unreachable!("only Solo steps support CopyTiming::Immediate");
                    };
                    let dm = roster.seats[seat].user.exclusive_available().await?;
                    let pctx = PromptCtx::new(
                        step.callout_name(),
                        step.effective_prompt_reads(),
                        seat,
                        step.eyes_open,
                        dm,
                        roster,
                        cfg_ref,
                        tier_deadline,
                        wakers_ref,
                        board_snap,
                    )
                    .as_immediate_copy();
                    let effect = prompt_fn(seat, pctx).await?;
                    Ok::<_, Error>((seat, step, effect))
                });

                let outcomes = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                if !immediate_seats.is_empty() {
                    wakers.wake_log.push((
                        tier.phase,
                        format!(
                            "{}: {}",
                            tier.callout_label(),
                            roster.format_seats(immediate_seats)
                        ),
                    ));
                }

                for (seat, step, effect) in outcomes {
                    wakers.acted_immediately.insert(seat);
                    let label = wakers.actor_label(roster, seat);
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                        room,
                        wakers: &mut wakers,
                    };
                    let mut bm = BoardMut::new(
                        &mut board,
                        step.callout_name(),
                        step.effective_prompt_reads(),
                        step.resolve_writes.0,
                    );
                    effect.apply(&label, &mut bm, &mut rctx);
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
                let step = *step;

                let all_actors = collect_solo_or_passive_actors(roster, step, &wakers);
                let slot_actors: &[usize] = if *slot_idx + 1 < *total_slots {
                    all_actors.get(*slot_idx..=*slot_idx).unwrap_or(&[])
                } else {
                    all_actors.get(*slot_idx..).unwrap_or(&[])
                };

                let board_snap = PromptBoard::new(&board);
                let wakers_ref = &wakers;
                let cfg_ref = &tier_cfg;

                let futs = slot_actors.iter().copied().map(|seat| async move {
                    let dm = roster.seats[seat].user.exclusive_available().await?;
                    let pctx = PromptCtx::new(
                        step.callout_name(),
                        step.effective_prompt_reads(),
                        seat,
                        step.eyes_open,
                        dm,
                        roster,
                        cfg_ref,
                        tier_deadline,
                        wakers_ref,
                        board_snap,
                    );
                    let effect = prompt_fn(seat, pctx).await?;
                    Ok::<_, Error>((seat, effect))
                });

                let outcomes = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                if !slot_actors.is_empty() {
                    wakers.wake_log.push((
                        tier.phase,
                        format!(
                            "{}: {}",
                            tier.callout_label(),
                            roster.format_seats(slot_actors.iter().copied())
                        ),
                    ));
                }

                for (seat, effect) in outcomes {
                    let label = wakers.actor_label(roster, seat);
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                        room,
                        wakers: &mut wakers,
                    };
                    let mut bm = BoardMut::new(
                        &mut board,
                        step.callout_name(),
                        step.effective_prompt_reads(),
                        step.resolve_writes.0,
                    );
                    effect.apply(&label, &mut bm, &mut rctx);
                }
            }

            TierKind::Steps(steps) => {
                let mut tier_step_wakers: Vec<(&'static str, SeatSet)> = Vec::new();
                for &step in steps {
                    let seats: SeatSet = match step.kind {
                        StepKind::Group(_) => compute_group_active_seats(step, &wakers),
                        StepKind::Passive(_) | StepKind::Solo(_) => {
                            collect_solo_or_passive_actors(roster, step, &wakers)
                                .into_iter()
                                .collect()
                        }
                    };
                    if step.eyes_open && !seats.is_empty() && step.order > 1 {
                        tier_step_wakers.push((step.callout_name(), seats));
                    }
                }

                let board_snap = PromptBoard::new(&board);
                let wakers_ref = &wakers;
                let cfg_ref = &tier_cfg;

                let mut futs: Vec<BoxFuture<'_, Result<StepTierResolution>>> = Vec::new();

                for &step in steps {
                    match step.kind {
                        StepKind::Group(prompt_fn) => {
                            let active_seats = compute_group_active_seats(step, wakers_ref);
                            if active_seats.is_empty() && !step.run_when_unheld {
                                continue;
                            }
                            futs.push(Box::pin(async move {
                                let gctx = GroupPromptCtx::new(
                                    step.callout_name(),
                                    step.effective_prompt_reads(),
                                    step.eyes_open,
                                    active_seats,
                                    roster,
                                    cfg_ref,
                                    tier_deadline,
                                    wakers_ref,
                                    board_snap,
                                );
                                let effect = prompt_fn(active_seats, gctx).await?;
                                Ok(StepTierResolution::Group { step, effect })
                            }));
                        }
                        StepKind::Passive(wake_fn) => {
                            for seat in collect_solo_or_passive_actors(roster, step, wakers_ref) {
                                futs.push(Box::pin(async move {
                                    Ok(StepTierResolution::Passive {
                                        step,
                                        seat,
                                        wake_fn,
                                    })
                                }));
                            }
                        }
                        StepKind::Solo(prompt_fn) => {
                            for seat in collect_solo_or_passive_actors(roster, step, wakers_ref) {
                                futs.push(Box::pin(async move {
                                    let dm = roster.seats[seat].user.exclusive_available().await?;
                                    let pctx = PromptCtx::new(
                                        step.callout_name(),
                                        step.effective_prompt_reads(),
                                        seat,
                                        step.eyes_open,
                                        dm,
                                        roster,
                                        cfg_ref,
                                        tier_deadline,
                                        wakers_ref,
                                        board_snap,
                                    );
                                    let effect = prompt_fn(seat, pctx).await?;
                                    Ok(StepTierResolution::Solo { step, seat, effect })
                                }));
                            }
                        }
                    }
                }

                let resolutions = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for (step_name, seats) in tier_step_wakers {
                    wakers.wake_log.push((
                        tier.phase,
                        format!("{step_name}: {}", roster.format_seats(seats)),
                    ));
                }

                for res in resolutions {
                    let table_summary = board.visible_table_summary(roster);
                    match res {
                        StepTierResolution::Group { step, effect } => {
                            let mut rctx = ResolveCtx {
                                roster,
                                chronicle: &mut all_chronicle,
                                room,
                                wakers: &mut wakers,
                            };
                            let mut bm = BoardMut::new(
                                &mut board,
                                step.callout_name(),
                                step.effective_prompt_reads(),
                                step.resolve_writes.0,
                            );
                            effect.apply(&mut bm, &mut rctx);
                        }
                        StepTierResolution::Passive {
                            step,
                            seat,
                            wake_fn,
                        } => {
                            let label = wakers.actor_label(roster, seat);
                            let mut rctx = ResolveCtx {
                                roster,
                                chronicle: &mut all_chronicle,
                                room,
                                wakers: &mut wakers,
                            };
                            if step.eyes_open {
                                if let Some(line) = table_summary {
                                    rctx.dm(seat, line);
                                }
                            }
                            let mut bm = BoardMut::new(
                                &mut board,
                                step.callout_name(),
                                BoardAccess::ALL_READS,
                                step.resolve_writes.0,
                            );
                            wake_fn(seat, &label, &mut bm, &mut rctx);
                        }
                        StepTierResolution::Solo { step, seat, effect } => {
                            let label = wakers.actor_label(roster, seat);
                            let mut rctx = ResolveCtx {
                                roster,
                                chronicle: &mut all_chronicle,
                                room,
                                wakers: &mut wakers,
                            };
                            let mut bm = BoardMut::new(
                                &mut board,
                                step.callout_name(),
                                step.effective_prompt_reads(),
                                step.resolve_writes.0,
                            );
                            effect.apply(&label, &mut bm, &mut rctx);
                        }
                    }
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
    for s in eligible {
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

//! Anti-Timing-Attack **Deck-Derived Fixed-Clock Tier Scheduler** for Dusk & Night.
//!
//! - [`build_deck_schedule`] derives the complete sequence of [`ScheduledTier`]s and their exact
//!   wall-clock offsets **purely from the public `deck` multiset** (never from dealt seats, center
//!   cards, copycat targets, `Mark::Fear`, or player response speed).
//! - Every step in a tier starts at `origin + tier.start_offset`, and [`run_night_phase`]
//!   unconditionally waits until `origin + tier.end_offset` (`tokio::time::sleep_until`) before
//!   committing mutations, sending passive/third-party DMs, or advancing to the next tier.

use std::collections::HashSet;
use std::time::Duration;

use futures::future::try_join_all;
use ircgm::prelude::*;
use tokio::time::{sleep_until, Instant};

use crate::board::{BoardAccess, BoardCards, BoardMut, PromptBoard, Roster};
use crate::marks::Mark;
pub use crate::role::Phase as TierPhase;
use crate::role::{
    BoxFuture, CopyTiming, GroupEffect, GroupPromptCtx, PassiveWakeFn, Phase, PromptCtx,
    ResolveCtx, Role, SeatEffect, StepKind, WakeStep,
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
    let deck_has_immediate_copy = deck.iter().any(|r| r.doppelganger_acts_immediately());
    let player_count = deck.len().saturating_sub(3).max(1);
    let copycat_count = deck.iter().filter(|&&r| r.id() == "copycat").count();
    let doppelganger_count = deck.iter().filter(|&&r| r.is_doppelganger()).count();

    let effective_order = |st: &'static WakeStep| -> u16 {
        if st.is_doppelganger {
            if has_dusk { 10 } else { 105 }
        } else {
            st.order
        }
    };
    let effective_phase = |st: &'static WakeStep| -> Phase {
        if has_dusk && effective_order(st) < 100 {
            Phase::Dusk
        } else {
            Phase::Night
        }
    };

    steps.sort_by_key(|&st| (effective_phase(st), effective_order(st), st.id()));

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

    for step in steps {
        let phase = effective_phase(step);

        if cur_phase != Some(phase) {
            flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
            cur_phase = Some(phase);
        }

        // Dedicated short-budget card-selection tiers (`Copycat`, `Doppelgänger`):
        if step.short_budget {
            flush_cur(&mut cur, cur_phase, &mut cur_writes, &mut raw_tiers);
            raw_tiers.push((
                phase,
                TierKind::Steps(vec![step]),
                timings.copycat_pick_budget,
            ));
            if step.is_doppelganger && deck_has_immediate_copy {
                raw_tiers.push((
                    phase,
                    TierKind::DoppelgangerImmediate(step),
                    timings.tier_budget,
                ));
            }
            continue;
        }

        if step.pack_faction().is_none() {
            let direct = deck.iter().filter(|&&r| r.has_step(step)).count();
            let cc_extra = if step.order > 8 { copycat_count } else { 0 };
            let holders_before_dg = direct.max(direct.saturating_sub(1) + cc_extra);
            let dg_extra = if step.copy_timing == Some(CopyTiming::OnRoleTurn) {
                doppelganger_count
            } else {
                0
            };
            let max_actors = (holders_before_dg + dg_extra).min(player_count);
            let eff_reads = step.effective_prompt_reads();
            if max_actors > 1
                && step.needs_dm_prompt()
                && (eff_reads & step.resolve_writes.0) != 0
            {
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
        label: String,
        wake_fn: PassiveWakeFn,
    },
    Solo {
        step: &'static WakeStep,
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
            Mark::finalize_dusk_boundary(
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

                let board_snap = PromptBoard::new(&board);
                let feared_snap = &feared_seats;
                let waking_snap = waking_roles.as_slice();
                let cfg_ref = &tier_cfg;
                let futs = immediate_seats.iter().map(|&seat| async move {
                    let step = waking_snap[seat]
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
                        feared_snap,
                        waking_snap,
                        board_snap,
                    )
                    .as_immediate_copy();
                    let effect = prompt_fn(seat, pctx).await?;
                    Ok::<_, Error>((seat, step, effect))
                });

                let outcomes = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for (seat, step, effect) in outcomes {
                    acted_immediately.insert(seat);
                    let label = waking_labels[seat].clone();
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                        waking_roles: &mut waking_roles,
                        waking_labels: &mut waking_labels,
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

                let all_actors = collect_solo_or_passive_actors(
                    roster,
                    step,
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

                let board_snap = PromptBoard::new(&board);
                let feared_snap = &feared_seats;
                let waking_snap = waking_roles.as_slice();
                let cfg_ref = &tier_cfg;

                let futs = slot_actors.iter().map(|(seat, label)| {
                    let seat = *seat;
                    let label = label.clone();
                    async move {
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
                            feared_snap,
                            waking_snap,
                            board_snap,
                        );
                        let effect = prompt_fn(seat, pctx).await?;
                        Ok::<_, Error>(StepTierResolution::Solo {
                            step,
                            label,
                            effect,
                        })
                    }
                });

                let resolutions = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for res in resolutions {
                    if let StepTierResolution::Solo {
                        step,
                        label,
                        effect,
                    } = res
                    {
                        let mut rctx = ResolveCtx {
                            roster,
                            chronicle: &mut all_chronicle,
                            waking_roles: &mut waking_roles,
                            waking_labels: &mut waking_labels,
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

            TierKind::Steps(steps) => {
                if steps.iter().any(|st| st.is_doppelganger) {
                    doppelganger_seats = (0..roster.len())
                        .filter(|&s| {
                            waking_roles[s].is_doppelganger() && !feared_seats.contains(&s)
                        })
                        .collect();
                }

                let board_snap = PromptBoard::new(&board);
                let waking_snap = waking_roles.as_slice();
                let feared_snap = &feared_seats;
                let cfg_ref = &tier_cfg;

                let mut futs: Vec<BoxFuture<'_, Result<StepTierResolution>>> = Vec::new();

                for &step in steps {
                    match step.kind {
                        StepKind::Group(prompt_fn) => {
                            let active_seats =
                                compute_group_active_seats(step, waking_snap, feared_snap);
                            if active_seats.is_empty() {
                                continue;
                            }
                            futs.push(Box::pin(async move {
                                let gctx = GroupPromptCtx::new(
                                    step.callout_name(),
                                    step.effective_prompt_reads(),
                                    step.eyes_open,
                                    bot,
                                    roster,
                                    cfg_ref,
                                    tier_deadline,
                                    waking_snap,
                                    feared_snap,
                                    board_snap,
                                );
                                let effect = prompt_fn(active_seats, gctx).await?;
                                Ok(StepTierResolution::Group { step, effect })
                            }));
                        }
                        StepKind::Passive(wake_fn) => {
                            for (seat, label) in collect_solo_or_passive_actors(
                                roster,
                                step,
                                waking_snap,
                                &waking_labels,
                                &acted_immediately,
                                feared_snap,
                            ) {
                                futs.push(Box::pin(async move {
                                    Ok(StepTierResolution::Passive {
                                        step,
                                        seat,
                                        label,
                                        wake_fn,
                                    })
                                }));
                            }
                        }
                        StepKind::Solo(prompt_fn) => {
                            for (seat, label) in collect_solo_or_passive_actors(
                                roster,
                                step,
                                waking_snap,
                                &waking_labels,
                                &acted_immediately,
                                feared_snap,
                            ) {
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
                                        feared_snap,
                                        waking_snap,
                                        board_snap,
                                    );
                                    let effect = prompt_fn(seat, pctx).await?;
                                    Ok(StepTierResolution::Solo {
                                        step,
                                        label,
                                        effect,
                                    })
                                }));
                            }
                        }
                    }
                }

                let resolutions = try_join_all(futs).await?;
                sleep_until(tier_deadline).await;

                for res in resolutions {
                    let table_summary = board.visible_table_summary(roster);
                    let mut rctx = ResolveCtx {
                        roster,
                        chronicle: &mut all_chronicle,
                        waking_roles: &mut waking_roles,
                        waking_labels: &mut waking_labels,
                    };
                    match res {
                        StepTierResolution::Group { step, effect } => {
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
                            label,
                            wake_fn,
                        } => {
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
                        StepTierResolution::Solo {
                            step,
                            label,
                            effect,
                        } => {
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
        Mark::finalize_dusk_boundary(
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

/// Returns `(seat, actor_label)` in resolution order: initial unfeared holders of `step` first,
/// followed by unfeared copiers (`Copycat` / `Doppelgänger` `X-A` sub-wakes) who haven't already acted.
fn collect_solo_or_passive_actors(
    roster: &Roster,
    step: &'static WakeStep,
    waking_roles: &[&'static dyn Role],
    waking_labels: &[String],
    acted_immediately: &HashSet<usize>,
    feared_seats: &HashSet<usize>,
) -> Vec<(usize, String)> {
    let is_eligible = |s: usize| {
        waking_roles[s].has_step(step)
            && !acted_immediately.contains(&s)
            && !feared_seats.contains(&s)
    };

    let mut actors: Vec<(usize, String)> = Vec::new();
    for s in 0..roster.len() {
        if is_eligible(s) && roster.seats[s].initial_role.has_step(step) {
            actors.push((s, waking_labels[s].clone()));
        }
    }
    for s in 0..roster.len() {
        if is_eligible(s) && !roster.seats[s].initial_role.has_step(step) {
            actors.push((s, waking_labels[s].clone()));
        }
    }
    actors
}

fn compute_group_active_seats(
    step: &'static WakeStep,
    waking_roles: &[&'static dyn Role],
    feared_seats: &HashSet<usize>,
) -> Vec<usize> {
    waking_roles
        .iter()
        .enumerate()
        .filter_map(|(s, r)| (!feared_seats.contains(&s) && r.has_step(step)).then_some(s))
        .collect()
}

#[cfg(test)]
#[path = "night_test.rs"]
mod test;

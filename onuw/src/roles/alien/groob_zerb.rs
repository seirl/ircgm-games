use ircgm::prelude::*;

use super::alien::ALIEN_PACK_STEP;
use crate::board::{BoardCards, SeatSet};
use crate::role::{
    EffectiveAlignment, GameSet, GroupEffect, GroupPromptCtx, NightFaction, Role, RoleInfo,
    SpecialVictory, Team, VictoryCtx, WakeStep,
};

pub struct Groob;
pub static GROOB: Groob = Groob;

pub struct Zerb;
pub static ZERB: Zerb = Zerb;

pub(super) const GROOB_ZERB_STEP: WakeStep = WakeStep::pair(
    "pair:groob_zerb",
    "Groob & Zerb",
    Team::Alien,
    Some(NightFaction::Alien),
    |seats, ctx| Box::pin(wake_groob_zerb_pair(seats, ctx)),
);

pub(super) fn both_groob_and_zerb_in_play(board: &BoardCards) -> bool {
    !board.active_holders(&GROOB).is_empty() && !board.active_holders(&ZERB).is_empty()
}

/// Computes `#3-C Leader` pointing vectors for waking `#1-D` `Groob` & `Zerb`
/// seats.
pub(super) fn groob_zerb_pointers(
    groob_awake: SeatSet,
    zerb_awake: SeatSet,
    copied_from: impl Fn(usize) -> Option<usize>,
) -> Vec<(usize, SeatSet)> {
    // Rule (ONUA Rulebook — Leader): "If both Zerb and Groob are in the game
    // and saw each other when they woke up, they point at each other (with
    // their eyes closed) instead of putting out their thumbs." Duplicate Groobs
    // without waking Zerb stay thumb-up Aliens; Doppelgänger skips
    // `copied_from`.
    if groob_awake.is_empty() || zerb_awake.is_empty() {
        return Vec::new();
    }
    let gz_awake = groob_awake | zerb_awake;
    gz_awake
        .into_iter()
        .filter_map(|s| {
            let mut targets = gz_awake.without(s);
            if let Some(src) = copied_from(s) {
                targets.remove(src);
            }
            (!targets.is_empty()).then_some((s, targets))
        })
        .collect()
}

async fn wake_groob_zerb_pair(
    active_seats: SeatSet,
    ctx: GroupPromptCtx<'_>,
) -> Result<GroupEffect> {
    let has_marks = ctx.roster.deck_has_marks();

    for s in active_seats {
        let is_groob = ctx.waking_roles[s].id() == "groob";
        let (self_name, rival_name, rival_in_deck) = if is_groob {
            ("Groob", "Zerb", ctx.roster.deck_has(&ZERB))
        } else {
            ("Zerb", "Groob", ctx.roster.deck_has(&GROOB))
        };

        let open_others = active_seats.without(s);
        let copied_src = ctx.copied_from(s).filter(|&src| open_others.contains(src));
        let mut rival_suspects = open_others;
        if let Some(src) = copied_src {
            rival_suspects.remove(src);
        }

        if !rival_in_deck {
            if open_others.is_empty() {
                ctx.dm(
                    s,
                    format!(
                        "👾 {self_name}: {rival_name} is not in the deck — you are on the regular Alien team tonight!"
                    ),
                );
            } else {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 {self_name}: {rival_name} is not in the deck — fellow {self_name}(s) {} also woke, and you are all on the regular Alien team tonight!",
                        ctx.roster.format_seats(open_others).bold().green(),
                    ),
                );
            }
        } else if open_others.is_empty() {
            let msg = if has_marks {
                format!("👾 {self_name}: Nobody else opened their eyes — if {rival_name} is in the center, you are on the regular Alien team tonight!")
            } else {
                format!("👾 {self_name}: {rival_name} is in the center — you are on the regular Alien team tonight!")
            };
            ctx.dm(s, msg);
        } else if rival_suspects.is_empty() {
            let src = copied_src.expect("open_others non-empty while rival_suspects empty");
            ctx.dm(
                s,
                irc_format!(
                    "👾 {self_name}: Only {} ({self_name}) opened their eyes — {rival_name} did not wake! If {rival_name} is in the center, you are on the regular Alien team tonight!",
                    ctx.nick(src).bold().green(),
                ),
            );
        } else if let Some(target) = rival_suspects.only() {
            if let Some(src) = copied_src {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 {self_name}: {} opened their eyes — since you copied {} ({self_name}), your mortal nemesis {rival_name} is {}! Survive and get {} killed to win!",
                        ctx.roster.format_seats(open_others).bold(),
                        ctx.nick(src).bold().green(),
                        ctx.nick(target).bold().red(),
                        ctx.nick(target).bold().red(),
                    ),
                );
            } else {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 {self_name}: {} opened their eyes across the table — your mortal nemesis {rival_name} is {}! Survive and get {} killed to win!",
                        ctx.nick(target).bold().red(),
                        ctx.nick(target).bold().red(),
                        ctx.nick(target).bold().red(),
                    ),
                );
            }
        } else {
            let suspect_list = ctx.roster.format_seats(rival_suspects);
            ctx.dm(
                s,
                irc_format!(
                    "👾 {self_name}: Multiple players opened their eyes ({}) — one is your nemesis {rival_name} and another is a copy! You point with both hands at {}; survive and get {rival_name} killed to win!",
                    suspect_list.as_str().bold().red(),
                    suspect_list.as_str().bold().red(),
                ),
            );
        }
    }

    Ok(GroupEffect::new(move |_board, rctx| {
        let names = rctx.roster.format_seats(active_seats);
        rctx.record(irc_format!(
            "👾 {} ({}) woke at #1D to look for each other.",
            "Groob & Zerb".bold().green(),
            names.bold(),
        ));
    }))
}

impl Role for Groob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "groob",
            "Groob",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: wake with the Aliens; if Zerb is in play, win ONLY if you survive and Zerb dies (otherwise win with the Aliens)!",
        )
        .night_faction(NightFaction::Alien)
        .threat_team(Team::Alien)
        .anarchy_bundle("groob_zerb")
        .non_village_labels(&["Alien"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP, GROOB_ZERB_STEP] }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        // Rule (ONUA Rulebook — Groob & Zerb): "Groob only wins if Zerb dies
        // and Groob stays alive... the village team wins as well, since at
        // least one alien was killed. Any other aliens in the game lose... If
        // only one of the two is in the game, she is on the alien team as
        // normal."
        cur.team = if both_groob_and_zerb_in_play(board) {
            Team::Independent
        } else {
            Team::Alien
        };
        cur
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !both_groob_and_zerb_in_play(ctx.board) || ctx.is_dead(seat) {
            return None;
        }
        // Rule (ONUA Doppelgänger — Groob/Zerb): "the real Groob and Zerb
        // may win by eliminating her if she is the Doppelgänger of their
        // target."
        ctx.active_holders(&ZERB)
            .intersects(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

impl Role for Zerb {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "zerb",
            "Zerb",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: wake with the Aliens; if Groob is in play, win ONLY if you survive and Groob dies (otherwise win with the Aliens)!",
        )
        .night_faction(NightFaction::Alien)
        .threat_team(Team::Alien)
        .anarchy_bundle("groob_zerb")
        .non_village_labels(&["Alien"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP, GROOB_ZERB_STEP] }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur.team = if both_groob_and_zerb_in_play(board) {
            Team::Independent
        } else {
            Team::Alien
        };
        cur
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !both_groob_and_zerb_in_play(ctx.board) || ctx.is_dead(seat) {
            return None;
        }
        ctx.active_holders(&GROOB)
            .intersects(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

#[cfg(test)]
#[path = "groob_zerb_test.rs"]
mod test;

//! Vampire Expansion **Vampire** — Silent Coven Pointing!
//!
//! Waking Vampires (`Vampire`, `The Master`, `Count`, Copycat/Doppelgänger-Vampire) recognize
//! each other and silently point at a non-Vampire target (`👉 <nick> points at <target>!`)
//! without talking at night.

use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, SeatSet};
use crate::marks::Mark;
use crate::pointing::{parse_single_player_point, silent_point};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep,
};

pub struct Vampire;
pub static VAMPIRE: Vampire = Vampire;

pub(crate) const VAMPIRE_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Vampire,
    25,
    |seats, ctx| Box::pin(Vampire::wake_group(seats, ctx)),
)
.dusk()
.writes(BoardAccess::MARKS);

impl Role for Vampire {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "vampire",
            "Vampire",
            GameSet::Vampire,
            Team::Vampire,
            "Wake in Dusk with the Vampire coven and bestow the Mark of the Vampire on a non-Vampire player!",
        )
        .aliases(&["vamp"])
        .night_faction(NightFaction::Vampire)
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[VAMPIRE_PACK_STEP] }
    }
}

impl Vampire {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let full_coven = ctx.seats_with_faction(NightFaction::Vampire);
        let non_vampires = ctx.roster.all_seats() - full_coven;

        if non_vampires.is_empty() {
            return Ok(GroupEffect::noop());
        }

        let coven_names = ctx.roster.format_seats(full_coven);
        let non_vamps_str = ctx.roster.format_seats(non_vampires);
        let roster = ctx.roster;

        let picked = silent_point(
            &ctx,
            active_seats,
            |_seat| {
                irc_format!(
                    "{} {} — Reply {} to point silently at a non-Vampire to turn! Non-Vampires: {non_vamps_str}",
                    "🧛 Vampire Coven:".bold().red(),
                    coven_names.as_str().bold(),
                    "<nick>".bold(),
                )
            },
            |_seat, raw| {
                parse_single_player_point(
                    roster,
                    raw,
                    "mark",
                    non_vampires,
                    SeatSet::EMPTY,
                    true,
                    "Pick a NON-Vampire player!",
                    "🤫 Silent night (no talking)! Reply <nick> to point at a non-Vampire.",
                )
            },
        )
        .await?;

        let (chosen, was_random) = match picked {
            Some((_actor, idx)) => (idx, false),
            None => (
                non_vampires.choose_random().expect("non_vampires non-empty"),
                true,
            ),
        };

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(active_seats, chosen, was_random, board, rctx);
        }))
    }

    fn resolve(
        active_seats: SeatSet,
        target: usize,
        was_random: bool,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        board.set_mark(target, Mark::Vampire);
        let target_nick = ctx.nick(target);
        if was_random || active_seats.len() >= 2 {
            for s in active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "🧛 Vampire Coven bestowed {} upon {}!",
                        Mark::Vampire.styled(),
                        target_nick.as_str().bold(),
                    ),
                );
            }
        }
        let actors = ctx.roster.format_seats(active_seats);
        ctx.record(irc_format!(
            "🧛 Vampire Coven ({}) bestowed {} on {}.",
            actors.bold().red(),
            Mark::Vampire.styled(),
            target_nick.bold(),
        ));
    }
}

#[cfg(test)]
#[path = "vampire_test.rs"]
mod test;

use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::marks::Mark;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, RoleTag, Team, Wake};

pub struct Renfield;
pub static RENFIELD: Renfield = Renfield;

impl Role for Renfield {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "renfield",
            "Renfield",
            GameSet::Vampire,
            Team::Vampire,
            "Start with the Mark of the Bat and wake in Dusk to see all Vampires AND whoever received the Mark of the Vampire!",
        )
        .minion()
        .tags(&[RoleTag::PlacesMarks])
    }

    fn on_deal(&self, seat: usize, board: &mut BoardCards, _ctx: &mut ResolveCtx<'_>) {
        board.marks[seat] = Mark::Bat;
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::passive(32, Self::inspect).dusk())
    }
}

impl Renfield {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let vamps: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| {
                board.players[i]
                    .effective_role()
                    .tags()
                    .contains(&RoleTag::VampirePack)
                    || board.marks[i] == Mark::Vampire
            })
            .collect();

        if vamps.is_empty() {
            if ctx.roster.deck_has_tanner_hazard() {
                ctx.dm(
                    seat,
                    "🦇 Renfield: No players are Vampires tonight! Survive and get another non-Tanner killed!",
                );
            } else {
                ctx.dm(
                    seat,
                    "🦇 Renfield: No players are Vampires tonight! Survive and get another player killed!",
                );
            }
        } else {
            ctx.dm(
                seat,
                irc_format!(
                    "🦇 Renfield: Your Vampire masters (including marked thrall) are: {}",
                    ctx.roster.format_indices(&vamps).bold().red()
                ),
            );
        }
        ctx.record(irc_format!(
            "🦇 {actor_label} ({}) took the {} and saw {} Vampire(s).",
            ctx.nick(seat).bold(),
            Mark::Bat.styled(),
            vamps.len(),
        ));
    }
}

#[cfg(test)]
#[path = "renfield_test.rs"]
mod test;

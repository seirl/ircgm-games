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
            "Wake in Dusk to see all Vampires and whoever received the Mark of the Vampire, then give yourself the Mark of the Bat! If no Vampires are in play, you are on the Village team.",
        )
        .minion()
        .tags(&[RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::passive(32, Self::inspect).dusk().mutates())
    }
}

impl Renfield {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let card_vamps: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| {
                ctx.waking_roles[i].tags().contains(&RoleTag::VampirePack)
                    || board.players[i]
                        .effective_role()
                        .tags()
                        .contains(&RoleTag::VampirePack)
            })
            .collect();
        let marked_vamps: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| board.marks[i] == Mark::Vampire)
            .collect();

        // Renfield places Mark of the Bat on himself at #-6C (overwriting any Mark::Vampire or Mark::Fear placed on him):
        board.marks[seat] = Mark::Bat;

        if card_vamps.is_empty() && marked_vamps.is_empty() {
            ctx.dm(
                seat,
                "🦇 Renfield: No players are Vampires tonight! You are on the Village team — win if no Villager dies!",
            );
        } else {
            let vamps_str = if card_vamps.is_empty() {
                "none".to_owned()
            } else {
                ctx.roster.format_indices(&card_vamps)
            };
            let thrall_str = if marked_vamps.is_empty() {
                "none".to_owned()
            } else {
                ctx.roster.format_indices(&marked_vamps)
            };
            ctx.dm(
                seat,
                irc_format!(
                    "🦇 Renfield: Vampires: {} · Marked with {}: {}",
                    vamps_str.as_str().bold().red(),
                    Mark::Vampire.styled(),
                    thrall_str.as_str().bold().red(),
                ),
            );
        }
        ctx.record(irc_format!(
            "🦇 {actor_label} ({}) placed {} on themselves and saw {} Vampire(s) + {} marked thrall(s).",
            ctx.nick(seat).bold(),
            Mark::Bat.styled(),
            card_vamps.len(),
            marked_vamps.len(),
        ));
    }
}

#[cfg(test)]
#[path = "renfield_test.rs"]
mod test;

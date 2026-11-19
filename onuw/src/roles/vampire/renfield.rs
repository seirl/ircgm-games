use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardCards, BoardKey, BoardMut, SeatSet};
use crate::marks::Mark;
use crate::role::{
    EffectiveAlignment, GameSet, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep,
};

#[derive(Clone, Copy, Debug, Default)]
struct RenfieldThrallSnapshot(Option<SeatSet>);

impl BoardKey for RenfieldThrallSnapshot {
    const ACCESS: u16 = BoardAccess::MARKS;

    fn init() -> Self {
        Self(None)
    }
}

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
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::passive(&RENFIELD, Self::inspect)
                .dusk()
                .writes(BoardAccess::MARKS)]
        }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        // Rule (ONUV — Renfield): "Renfield is on the Vampire team (though he
        // is not a Vampire himself), and wins if no Vampires die (even if
        // Renfield himself dies). If no players are Vampires (or have the Mark
        // of the Vampire), Renfield is on the villager team."
        if !(0..board.player_count()).any(|i| board.is_monster_threat_for(i, Team::Vampire)) {
            cur.team = Team::Village;
        }
        cur
    }
}

impl Renfield {
    fn inspect(seat: usize, actor_label: &str, board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        // Rule (ONUV — Renfield): "Renfield wakes up, sees who the Vampires
        // are and who they are pointing at (who now has the Mark of the
        // Vampire), and then places the Mark of the Bat in front of himself..."
        // (snapshotting `Mark::Vampire` before `Mark::Bat` can overwrite
        // `seat`).
        let card_vamps: SeatSet = ctx
            .roster
            .all_seats()
            .into_iter()
            .filter(|&i| ctx.waking_role(i).night_faction() == Some(NightFaction::Vampire))
            .collect();
        let marked_vamps: SeatSet = match board.get_key::<RenfieldThrallSnapshot>().0 {
            Some(snap) => snap,
            None => {
                let snap: SeatSet = ctx
                    .roster
                    .all_seats()
                    .into_iter()
                    .filter(|&i| board.mark_at(i) == Mark::Vampire)
                    .collect();
                board.set_key(RenfieldThrallSnapshot(Some(snap)));
                snap
            }
        };

        // Renfield places Mark of the Bat on himself at #-6C (overwriting any Mark::Vampire or Mark::Fear placed on him):
        board.set_mark(seat, Mark::Bat);

        if card_vamps.is_empty() && marked_vamps.is_empty() {
            ctx.dm(
                seat,
                "🦇 Renfield: No players are Vampires tonight! You are on the Village team — win if no Villager dies!",
            );
        } else {
            let vamps_str = if card_vamps.is_empty() {
                "none".to_owned()
            } else {
                ctx.roster.format_seats(card_vamps)
            };
            let thrall_str = if marked_vamps.is_empty() {
                "none".to_owned()
            } else {
                ctx.roster.format_seats(marked_vamps)
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

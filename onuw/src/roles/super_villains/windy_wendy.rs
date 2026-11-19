use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{GameSet, Role, RoleInfo, Team};

pub struct WindyWendy;
pub static WINDY_WENDY: WindyWendy = WindyWendy;

struct ConvertedVillain;
static CONVERTED_VILLAIN: ConvertedVillain = ConvertedVillain;

impl Role for ConvertedVillain {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "windy_wendy_villain",
            "Super Villain",
            GameSet::SuperVillains,
            Team::SuperVillain,
            "Windy Wendy swayed onto the Super Villain team by a Villain's vote.",
        )
    }
}

impl Role for WindyWendy {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "windy_wendy",
            "Windy Wendy",
            GameSet::SuperVillains,
            Team::Village,
            "Start on the Hero/Village team and sleep at night. If at least one Super Villain points at you during the Vote, you join the Super Villain team!",
        )
        .aliases(&["windywendy", "wendy", "windy"])
    }

    fn vote_conversion(
        &self,
        seat: usize,
        roster: &Roster,
        board: &BoardCards,
        votes: &[usize],
    ) -> Option<(&'static dyn Role, Text)> {
        // Rule (Bonus Roles BP4 — Windy Wendy): "Windy Wendy is on the hero
        // team, unless at least one villain votes for her, and then she becomes
        // a villain... if Windy Wendy has the most players pointing at her, and
        // at least one of those players is a villain, a villain has been
        // caught."
        let villain_voted = votes.iter().enumerate().any(|(v, &target)| {
            v != seat && target == seat && board.is_monster_threat_for(v, Team::SuperVillain)
        });
        if villain_voted {
            Some((
                &CONVERTED_VILLAIN,
                irc_format!(
                    "💨 {} ({}) was pointed at by a Super Villain and switched to the {} team!",
                    roster.seats[seat].user.nick().bold(),
                    self.info().styled_name(),
                    CONVERTED_VILLAIN.info().styled_name(),
                ),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "windy_wendy_test.rs"]
mod test;

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{GameSet, Role, RoleInfo, Team};

pub struct Cursed;
pub static CURSED: Cursed = Cursed;

struct CursedWerewolf;
static CURSED_WEREWOLF: CursedWerewolf = CursedWerewolf;

impl Role for CursedWerewolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cursed_werewolf",
            "Werewolf",
            GameSet::Daybreak,
            Team::Werewolf,
            "Cursed villager turned into a Werewolf by a Werewolf vote.",
        )
    }
}

struct CursedVampire;
static CURSED_VAMPIRE: CursedVampire = CursedVampire;

impl Role for CursedVampire {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cursed_vampire",
            "Vampire",
            GameSet::Daybreak,
            Team::Vampire,
            "Cursed villager turned into a Vampire by a Vampire vote.",
        )
    }
}

impl Role for Cursed {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cursed",
            "Cursed",
            GameSet::Daybreak,
            Team::Village,
            "Start on the Village team and sleep at night. If at least one Werewolf (or Vampire) points at you during the Vote, you turn into a Werewolf (or Vampire)!",
        )
    }

    fn vote_conversion(
        &self,
        seat: usize,
        roster: &Roster,
        board: &BoardCards,
        votes: &[Option<usize>],
    ) -> Option<(&'static dyn Role, Text)> {
        let werewolf_voted = votes
            .iter()
            .enumerate()
            .any(|(v, &opt)| opt == Some(seat) && board.is_monster_threat_for(v, Team::Werewolf));
        if werewolf_voted {
            return Some((
                &CURSED_WEREWOLF,
                irc_format!(
                    "🐺 {} ({}) was pointed at by a Werewolf and turned into a {}!",
                    roster.seats[seat].user.nick().bold(),
                    self.info().styled_name(),
                    CURSED_WEREWOLF.info().styled_name(),
                ),
            ));
        }

        let vampire_voted = votes
            .iter()
            .enumerate()
            .any(|(v, &opt)| opt == Some(seat) && board.is_monster_threat_for(v, Team::Vampire));
        if vampire_voted {
            return Some((
                &CURSED_VAMPIRE,
                irc_format!(
                    "🧛 {} ({}) was pointed at by a Vampire and turned into a {}!",
                    roster.seats[seat].user.nick().bold(),
                    self.info().styled_name(),
                    CURSED_VAMPIRE.info().styled_name(),
                ),
            ));
        }

        None
    }
}

#[cfg(test)]
#[path = "cursed_test.rs"]
mod test;

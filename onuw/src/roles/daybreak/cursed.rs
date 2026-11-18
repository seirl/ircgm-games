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
        let mut ww_votes = 0usize;
        let mut vamp_votes = 0usize;
        for (v, &opt) in votes.iter().enumerate() {
            if v != seat && opt == Some(seat) {
                if board.is_monster_threat_for(v, Team::Werewolf) {
                    ww_votes += 1;
                }
                if board.is_monster_threat_for(v, Team::Vampire) {
                    vamp_votes += 1;
                }
            }
        }
        if ww_votes == 0 && vamp_votes == 0 {
            return None;
        }
        let turn_werewolf = if ww_votes != vamp_votes {
            ww_votes > vamp_votes
        } else {
            fastrand::bool()
        };
        if turn_werewolf {
            Some((
                &CURSED_WEREWOLF,
                irc_format!(
                    "🐺 {} ({}) was pointed at by a Werewolf and turned into a {}!",
                    roster.seats[seat].user.nick().bold(),
                    self.info().styled_name(),
                    CURSED_WEREWOLF.info().styled_name(),
                ),
            ))
        } else {
            Some((
                &CURSED_VAMPIRE,
                irc_format!(
                    "🧛 {} ({}) was pointed at by a Vampire and turned into a {}!",
                    roster.seats[seat].user.nick().bold(),
                    self.info().styled_name(),
                    CURSED_VAMPIRE.info().styled_name(),
                ),
            ))
        }
    }
}

#[cfg(test)]
#[path = "cursed_test.rs"]
mod test;

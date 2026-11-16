use ircgm::prelude::*;

use super::vampire::VAMPIRE_PACK_STEP;
use crate::role::{
    DeathCtx, GameSet, NightFaction, Role, RoleInfo, Team, VoteProtection, WakeStep,
};

pub struct TheMaster;
pub static THE_MASTER: TheMaster = TheMaster;

impl Role for TheMaster {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "the_master",
            "The Master",
            GameSet::Vampire,
            Team::Vampire,
            "Wake with the Vampire coven. If at least one Vampire points at you during the Day vote, you CANNOT die!",
        )
        .aliases(&["master"])
        .night_faction(NightFaction::Vampire)
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[VAMPIRE_PACK_STEP] }
    }

    fn vote_protection(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteProtection> {
        let supported = (0..ctx.roster.len()).any(|voter| {
            voter != seat
                && ctx.votes[voter] == Some(seat)
                && ctx.board.is_monster_threat_for(voter, Team::Vampire)
        });
        supported.then(|| VoteProtection {
            protected_seat: seat,
            blocks_hunter_shot: true,
            announcement: irc_format!(
                "🧛 {} ({}) was shielded by a Vampire's loyalty vote and cannot die!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
            ),
        })
    }
}

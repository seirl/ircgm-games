use ircgm::prelude::*;

use crate::role::{DeathCtx, DeathEffect, GameSet, Role, RoleInfo, Team};

pub struct Hunter;
pub static HUNTER: Hunter = Hunter;

impl Role for Hunter {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "hunter",
            "Hunter",
            GameSet::Base,
            Team::Village,
            "Sleep at night. If you are killed, whoever YOU pointed at also dies immediately!",
        )
    }

    fn on_death(&self, dead_seat: usize, ctx: &DeathCtx<'_>) -> Option<DeathEffect> {
        let target_seat = ctx.votes[dead_seat]?;
        let hunter_nick = ctx.roster.seats[dead_seat].user.nick();
        let target_nick = ctx.roster.seats[target_seat].user.nick();
        Some(DeathEffect::Kill {
            seat: target_seat,
            announcement: irc_format!(
                "{} {} died as Hunter and took down {}!",
                "🏹 HUNTER SHOT!".bold().orange(),
                hunter_nick.bold(),
                target_nick.bold().red(),
            ),
        })
    }
}

#[cfg(test)]
#[path = "hunter_test.rs"]
mod test;

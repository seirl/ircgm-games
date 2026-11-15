use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Drunk;
pub static DRUNK: Drunk = Drunk;

impl Role for Drunk {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "drunk",
            "Drunk",
            GameSet::Base,
            Team::Village,
            "Wake and MUST swap your card face-down with 1 of the 3 center cards without looking.",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(180, Self::prompt).mutates())
    }
}

impl Drunk {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let ans = ctx
                .ask(
                    irc_format!(
                        "🍺 Drunk: You MUST swap your card with center {}, {}, or {}!",
                        "1".bold(),
                        "2".bold(),
                        "3".bold(),
                    ),
                    |m| match Roster::parse_center_slot(&m.text) {
                        Some(c_idx) => AskMatch::accept_say(
                            c_idx,
                            format!("🍺 Locked in face-down swap with Center #{}!", c_idx + 1),
                        ),
                        None => {
                            AskMatch::reject("Drunk MUST swap! Reply 1, 2, or 3.".yellow())
                        }
                    },
                )
                .await?;

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, ans, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<usize>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let c_idx = match ans {
            Answer::Ok(c) => c,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let auto = fastrand::usize(0..3);
                ctx.dm(
                    seat,
                    format!("🍺 Randomly swapped your card with Center #{}.", auto + 1),
                );
                auto
            }
        };
        if board.is_shielded(seat) {
            ctx.dm(seat, "🛡️ Your card was shielded—Drunk swap prevented!".yellow());
            return;
        }
        board.record_card_interaction(seat);
        let old_self = board.players[seat];
        let new_self = board.center[c_idx];
        std::mem::swap(&mut board.players[seat], &mut board.center[c_idx]);
        ctx.record(irc_format!(
            "🍺 {actor_label} ({}) swapped {} with Center #{} ({}).",
            ctx.nick(seat).bold(),
            old_self.display_for_peek(),
            c_idx + 1,
            new_self.display_for_peek(),
        ));
    }
}

#[cfg(test)]
#[path = "drunk_test.rs"]
mod test;

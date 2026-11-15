use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct ApprenticeSeer;
pub static APPRENTICE_SEER: ApprenticeSeer = ApprenticeSeer;

impl Role for ApprenticeSeer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "apprentice_seer",
            "Apprentice Seer",
            GameSet::Daybreak,
            Team::Village,
            "Wake to view 1 of the 3 center cards.",
        )
        .aliases(&["aseer", "appseer"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(152, Self::prompt))
    }
}

impl ApprenticeSeer {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let ans = ctx
                .ask(
                    "🔮 Apprentice Seer: Reply `1`, `2`, or `3` to view a center card, or `pass`:",
                    |m| {
                        if Roster::is_pass(&m.text) {
                            return AskMatch::accept(None);
                        }
                        match Roster::parse_center_slot(&m.text) {
                            Some(c_idx) => AskMatch::accept(Some(c_idx)),
                            None => AskMatch::reject("Reply `1`, `2`, `3`, or `pass`.".yellow()),
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
        ans: Answer<Option<usize>>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(c_idx)) = ans {
            board.record_card_interaction(seat);
            let seen = board.center[c_idx];
            ctx.dm(
                seat,
                irc_format!("🔮 Center #{} is: {}", c_idx + 1, seen.display_for_peek()),
            );
            ctx.record(irc_format!(
                "🔮 {actor_label} ({}) viewed Center #{} ({}).",
                ctx.nick(seat).bold(),
                c_idx + 1,
                seen.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "apprentice_seer_test.rs"]
mod test;

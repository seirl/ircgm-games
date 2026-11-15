use ircgm::prelude::*;

use crate::role::{BoxFuture, GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, Team, Wake};

pub struct Mason;
pub static MASON: Mason = Mason;

impl Role for Mason {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mason",
            "Mason",
            GameSet::Base,
            Team::Village,
            "Wake to see the other Mason. If you are alone, the other Mason card is in the center.",
        )
        .paired()
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(140, Self::wake_group))
    }
}

impl Mason {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            if active_seats.len() == 1 {
                ctx.dm(
                    active_seats[0],
                    irc_format!(
                        "🧱 Mason: You are the {} in play (the other Mason card is in the center!).",
                        "ONLY Mason".bold().light_cyan()
                    ),
                );
            } else {
                for &seat in &active_seats {
                    let others: Vec<usize> = active_seats
                        .iter()
                        .copied()
                        .filter(|&o| o != seat)
                        .collect();
                    ctx.dm(
                        seat,
                        irc_format!(
                            "🧱 Fellow Mason(s): {}",
                            ctx.roster.format_indices(&others).bold().light_cyan()
                        ),
                    );
                }
            }

            Ok(GroupEffect::new(move |_board, rctx| {
                if active_seats.len() == 1 {
                    rctx.record(irc_format!(
                        "🧱 {} ({}) woke alone (the other Mason was in the center).",
                        MASON.info().styled_name(),
                        rctx.nick(active_seats[0]).bold(),
                    ));
                } else {
                    let names = rctx.roster.format_indices(&active_seats);
                    rctx.record(irc_format!(
                        "🧱 Masons ({}) recognized each other.",
                        names.bold().light_cyan(),
                    ));
                }
            }))
        })
    }
}

#[cfg(test)]
#[path = "mason_test.rs"]
mod test;

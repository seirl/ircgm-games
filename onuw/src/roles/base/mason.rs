use ircgm::prelude::*;

use crate::role::{GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, Team, Wake};

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
        Some(Wake::group(140, |seats, ctx| {
            Box::pin(Self::wake_group(seats, ctx))
        }))
    }
}

impl Mason {
    async fn wake_group(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let total_masons = ctx.seats_with_role(&MASON).len();
        if active_seats.len() == 1 {
            if total_masons > 1 {
                ctx.dm(
                    active_seats[0],
                    irc_format!(
                        "🧱 Mason: You woke {} (your fellow Mason could not wake up tonight!).",
                        "alone".bold().light_cyan()
                    ),
                );
            } else {
                ctx.dm(
                    active_seats[0],
                    irc_format!(
                        "🧱 Mason: You are the {} in play (the other Mason card is in the center!).",
                        "ONLY Mason".bold().light_cyan()
                    ),
                );
            }
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
                let reason = if total_masons > 1 {
                    "fellow Mason feared"
                } else {
                    "the other Mason was in the center"
                };
                rctx.record(irc_format!(
                    "🧱 {} ({}) woke alone ({reason}).",
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
    }
}

#[cfg(test)]
#[path = "mason_test.rs"]
mod test;

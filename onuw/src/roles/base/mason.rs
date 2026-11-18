use ircgm::prelude::*;

use crate::board::SeatSet;
use crate::role::{GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, Team, WakeStep};

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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::group(&MASON, |seats, ctx| {
                Box::pin(Self::wake_group(seats, ctx))
            })
            .copy_with_group()]
        }
    }
}

impl Mason {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let total_masons = ctx.seats_with_role(&MASON).len();
        let has_pre_mason_center_swap =
            ctx.roster.deck_has_marks() || ctx.roster.deck.iter().any(|r| r.id() == "oracle");
        if let Some(solo) = active_seats.only() {
            if has_pre_mason_center_swap {
                ctx.dm(
                    solo,
                    irc_format!(
                        "🧱 Mason: You woke {} tonight (no other Mason opened their eyes!).",
                        "alone".bold().light_cyan()
                    ),
                );
            } else {
                ctx.dm(
                    solo,
                    irc_format!(
                        "🧱 Mason: You are the {} in play (the other Mason card is in the center!).",
                        "ONLY Mason".bold().light_cyan()
                    ),
                );
            }
        } else {
            for seat in active_seats {
                let others = active_seats.without(seat);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🧱 Fellow Mason(s): {}",
                        ctx.roster.format_seats(others).bold().light_cyan()
                    ),
                );
            }
        }

        Ok(GroupEffect::new(move |_board, rctx| {
            if let Some(solo) = active_seats.only() {
                let reason = if total_masons > 1 {
                    "fellow Mason feared"
                } else {
                    "the other Mason was in the center"
                };
                rctx.record(irc_format!(
                    "🧱 {} ({}) woke alone ({reason}).",
                    MASON.info().styled_name(),
                    rctx.nick(solo).bold(),
                ));
            } else {
                let names = rctx.roster.format_seats(active_seats);
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

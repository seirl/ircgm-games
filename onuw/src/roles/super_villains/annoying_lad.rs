use ircgm::prelude::*;

use crate::board::Roster;
use crate::role::{GameSet, PromptCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct AnnoyingLad;
pub static ANNOYING_LAD: AnnoyingLad = AnnoyingLad;

impl Role for AnnoyingLad {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "annoying_lad",
            "Annoying Lad",
            GameSet::SuperVillains,
            Team::Village,
            "May nudge either your immediate left or right neighbor so they know Annoying Lad sits beside them!",
        )
        .aliases(&["lad", "annoying"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&ANNOYING_LAD, 142, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .immediate_copy(),
            ]
        }
    }
}

impl AnnoyingLad {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let (left, right) = roster.neighbors(seat);
        let prompt = irc_format!(
            "😜 Annoying Lad: Reply {} ({}), {} ({}), or {} to poke a neighbor!",
            "left".bold(),
            ctx.nick(left),
            "right".bold(),
            ctx.nick(right),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "😜 Passed — stayed still tonight.");
                }
                match m.text.trim().to_ascii_lowercase().as_str() {
                    "left" | "l" => AskMatch::accept_say(
                        Some(left),
                        format!("😜 Poked {}!", roster.seats[left].user.nick()),
                    ),
                    "right" | "r" => AskMatch::accept_say(
                        Some(right),
                        format!("😜 Poked {}!", roster.seats[right].user.nick()),
                    ),
                    other => match roster.resolve_nick(other) {
                        Some(idx) if idx == left || idx == right => AskMatch::accept_say(
                            Some(idx),
                            format!("😜 Poked {}!", roster.seats[idx].user.nick()),
                        ),
                        _ => AskMatch::reject(
                            "Pick left, right (or your neighbor's nickname), or pass!".yellow(),
                        ),
                    },
                }
            })
            .await?;

        Ok(SeatEffect::new(move |label, _board, rctx| {
            if let Answer::Ok(Some(target)) = ans {
                let (left, _) = rctx.roster.neighbors(seat);
                let shoulder = if target == left { "right" } else { "left" };
                let side = if target == left { "right" } else { "left" };
                rctx.dm(
                    target,
                    irc_format!(
                        "{} Someone poked your {shoulder} shoulder — your immediate {side} neighbor is the Annoying Lad!",
                        "👉 *POKE!*".bold().yellow(),
                    ),
                );
                rctx.record(irc_format!(
                    "😜 {label} ({}) poked neighbor {}.",
                    rctx.nick(seat).bold(),
                    rctx.nick(target).bold(),
                ));
            } else {
                rctx.record(irc_format!(
                    "😜 {label} ({}) stayed still.",
                    rctx.nick(seat).bold(),
                ));
            }
        }))
    }
}

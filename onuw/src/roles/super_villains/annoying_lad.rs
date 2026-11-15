use ircgm::prelude::*;

use crate::role::{BoxFuture, GameSet, PromptCtx, Role, RoleInfo, SeatEffect, Team, Wake};

pub struct AnnoyingLad;
pub static ANNOYING_LAD: AnnoyingLad = AnnoyingLad;

impl Role for AnnoyingLad {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "annoying_lad",
            "Annoying Lad",
            GameSet::SuperVillains,
            Team::Village,
            "Nudge either your immediate left or right neighbor in DM so they know Annoying Lad sits beside them!",
        )
        .aliases(&["lad", "annoying"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(142, Self::prompt))
    }
}

impl AnnoyingLad {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let (left, right) = roster.neighbors(seat);
            let prompt = irc_format!(
                "😜 Annoying Lad: Reply {} ({}) or {} ({}) to poke a neighbor!",
                "left".bold(),
                ctx.nick(left),
                "right".bold(),
                ctx.nick(right),
            );
            let ans = ctx
                .ask(
                    prompt,
                    move |m| match m.text.trim().to_ascii_lowercase().as_str() {
                        "left" | "l" => AskMatch::accept_say(
                            left,
                            format!("😜 Poked {}!", roster.seats[left].user.nick()),
                        ),
                        "right" | "r" => AskMatch::accept_say(
                            right,
                            format!("😜 Poked {}!", roster.seats[right].user.nick()),
                        ),
                        other => match roster.resolve_nick(other) {
                            Some(idx) if idx == left || idx == right => AskMatch::accept_say(
                                idx,
                                format!("😜 Poked {}!", roster.seats[idx].user.nick()),
                            ),
                            _ => AskMatch::reject(
                                "Pick left or right (or your neighbor's nickname)!".yellow(),
                            ),
                        },
                    },
                )
                .await?;

            Ok(SeatEffect::new(move |label, _board, rctx| {
                let target = ans.ok().unwrap_or(left);
                rctx.dm(
                    target,
                    irc_format!(
                        "{} One of your immediate neighbors is the Annoying Lad!",
                        "👉 *POKE!*".bold().yellow(),
                    ),
                );
                rctx.record(irc_format!(
                    "😜 {label} ({}) poked neighbor {}.",
                    rctx.nick(seat).bold(),
                    rctx.nick(target).bold(),
                ));
            }))
        })
    }
}

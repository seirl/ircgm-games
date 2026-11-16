use ircgm::prelude::*;

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
            "Must nudge either your immediate left or right neighbor so they know Annoying Lad sits beside them!",
        )
        .aliases(&["lad", "annoying"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&ANNOYING_LAD, 142, |s, ctx| {
                Box::pin(Self::prompt(s, ctx))
            })]
        }
    }
}

impl AnnoyingLad {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let (left, right) = roster.neighbors(seat);
        let prompt = irc_format!(
            "😜 Annoying Lad: You MUST poke your {} ({}) or {} ({}) neighbor!",
            "left".bold(),
            ctx.nick(left),
            "right".bold(),
            ctx.nick(right),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                let raw = m.text.trim();
                match raw.to_ascii_lowercase().as_str() {
                    "left" | "l" => AskMatch::accept_say(
                        left,
                        format!("😜 Poked {}!", roster.seats[left].user.nick()),
                    ),
                    "right" | "r" => AskMatch::accept_say(
                        right,
                        format!("😜 Poked {}!", roster.seats[right].user.nick()),
                    ),
                    _ => match roster.resolve_nick(raw) {
                        Some(idx) if idx == left || idx == right => AskMatch::accept_say(
                            idx,
                            format!("😜 Poked {}!", roster.seats[idx].user.nick()),
                        ),
                        _ => AskMatch::reject(
                            "You MUST pick left or right (or your immediate neighbor's nickname)!"
                                .yellow(),
                        ),
                    },
                }
            })
            .await?;

        Ok(SeatEffect::new(move |label, _board, rctx| {
            let (left, right) = rctx.roster.neighbors(seat);
            let (target, was_random) = match ans {
                Answer::Ok(idx) => (idx, false),
                Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                    let auto = if fastrand::bool() { left } else { right };
                    (auto, true)
                }
            };
            if was_random {
                rctx.dm(
                    seat,
                    format!("😜 Time's up — automatically poked {}!", rctx.nick(target)),
                );
            }
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
        }))
    }
}

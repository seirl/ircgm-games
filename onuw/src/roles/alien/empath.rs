use futures::future::try_join_all;
use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, SeatSet};
use crate::role::{GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, Team, WakeStep};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EmpathQuestion {
    MostSuspicious,
    MostTrustworthy,
    LikelyEvil,
    LikelyWinner,
}

impl EmpathQuestion {
    fn roll() -> Self {
        match fastrand::u8(..4) {
            0 => Self::MostSuspicious,
            1 => Self::MostTrustworthy,
            2 => Self::LikelyEvil,
            _ => Self::LikelyWinner,
        }
    }

    fn phrase(self) -> &'static str {
        match self {
            Self::MostSuspicious => "the player you find MOST SUSPICIOUS",
            Self::MostTrustworthy => "the player you TRUST THE MOST",
            Self::LikelyEvil => "the player most likely to be EVIL",
            Self::LikelyWinner => "the player most likely to WIN",
        }
    }

    fn short_label(self) -> &'static str {
        match self {
            Self::MostSuspicious => "most suspicious",
            Self::MostTrustworthy => "most trustworthy",
            Self::LikelyEvil => "likely Evil",
            Self::LikelyWinner => "likely winner",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedEmpathQuestion(pub Option<EmpathQuestion>);

impl BoardKey for ForcedEmpathQuestion {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

pub struct Empath;
pub static EMPATH: Empath = Empath;

impl Role for Empath {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "empath",
            "Empath",
            GameSet::Alien,
            Team::Village,
            "At the end of Night, everyone else keeps their eyes closed and points at a player for a Mother Ship poll — Empath opens their eyes to see who everyone points at!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::group(&EMPATH, |seats, ctx| Box::pin(Self::wake_group(seats, ctx)))
                .run_when_unheld()
                .writes(BoardAccess::PUBLIC_ANNOUNCE)]
        }
    }
}

impl Empath {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let question = ctx
            .get_key::<ForcedEmpathQuestion>()
            .0
            .unwrap_or_else(EmpathQuestion::roll);

        for e_seat in active_seats {
            ctx.dm(
                e_seat,
                irc_format!(
                    "💓 Empath: Open your eyes! Everyone else is keeping their eyes closed and pointing at {} — results reveal when the phase ends!",
                    question.phrase().bold().yellow(),
                ),
            );
        }

        let pointers = ctx.roster.all_seats() - active_seats;
        let roster = ctx.roster;
        let ctx_ref = &ctx;

        let futs = pointers.into_iter().map(|p_seat| async move {
            let others_str = roster.format_seats(roster.all_seats().without(p_seat));
            let prompt = irc_format!(
                "🙈 Empath Poll (keep your eyes closed!): Point at {} by replying {}! Other players: {others_str}",
                question.phrase().bold().yellow(),
                "<nick>".bold(),
            );
            let ans = ctx_ref
                .ask(p_seat, prompt, move |m| match roster.resolve_nick(&m.text) {
                    Some(idx) if idx == p_seat => {
                        AskMatch::reject("Point at ANOTHER player, not yourself!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "👉 Pointing at {} (eyes closed).",
                            roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Reply with another player's nickname.".yellow()),
                })
                .await?;
            let chosen = match ans {
                Answer::Ok(idx) => idx,
                _ => roster
                    .all_seats()
                    .without(p_seat)
                    .choose_random()
                    .expect("at least 2 players"),
            };
            Ok::<_, Error>((p_seat, chosen))
        });

        let results = try_join_all(futs).await?;

        Ok(GroupEffect::new(move |_board, rctx| {
            rctx.announce(irc_format!(
                "💓 {} Everyone pointed at {}!",
                "Mother Ship (Empath):".bold().cyan(),
                question.phrase().bold().yellow(),
            ));
            let pairs_str = results
                .iter()
                .map(|&(src, dst)| format!("{} → {}", rctx.nick(src), rctx.nick(dst)))
                .collect::<Vec<_>>()
                .join(", ");

            for e_seat in active_seats {
                if pairs_str.is_empty() {
                    rctx.dm(e_seat, "💓 Empath: No other players pointed.");
                } else {
                    rctx.dm(
                        e_seat,
                        irc_format!(
                            "💓 Empath Poll ({}): {}",
                            question.short_label(),
                            pairs_str.as_str().bold(),
                        ),
                    );
                }
            }

            if active_seats.is_empty() {
                rctx.record(irc_format!(
                    "💓 Empath poll ({}) ran with Empath in the center ({pairs_str}).",
                    question.short_label(),
                ));
            } else {
                let empath_names = rctx.roster.format_seats(active_seats);
                rctx.record(irc_format!(
                    "💓 {} ({}) observed poll ({}): {pairs_str}.",
                    EMPATH.info().styled_name(),
                    empath_names.bold(),
                    question.short_label(),
                ));
            }
        }))
    }
}

#[cfg(test)]
#[path = "empath_test.rs"]
mod test;

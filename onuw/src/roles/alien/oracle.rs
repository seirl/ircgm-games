use ircgm::prelude::*;

use crate::board::OracleOmen;
use crate::role::{BoxFuture, GameSet, PromptCtx, Role, RoleInfo, SeatEffect, Team, Wake};

pub struct Oracle;
pub static ORACLE: Oracle = Oracle;

impl Role for Oracle {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "oracle",
            "Oracle",
            GameSet::Alien,
            Team::Village,
            "Answer a Mother-Ship question during Night that alters how downstream Alien-set roles behave!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(142, Self::prompt).mutates())
    }
}

impl Oracle {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let ans = ctx
                .ask(
                    "🔮🛸 Oracle: The Mother Ship asks — tune cosmic frequencies to `amplify` (boosts Psychic visions), `static`, or `normal`?",
                    |m| match m.text.trim().to_ascii_lowercase().as_str() {
                        "amplify" | "a" | "yes" => AskMatch::accept_say(
                            OracleOmen::AmplifiedSignals,
                            "🔮🛸 Cosmic signals AMPLIFIED!",
                        ),
                        "static" | "s" => AskMatch::accept_say(
                            OracleOmen::QuantumStatic,
                            "🔮🛸 Quantum STATIC engaged!",
                        ),
                        "normal" | "n" | "pass" | "skip" => AskMatch::accept_say(
                            OracleOmen::Normal,
                            "🔮🛸 Signals kept NORMAL.",
                        ),
                        _ => AskMatch::reject("Reply `amplify`, `static`, or `normal`.".yellow()),
                    },
                )
                .await?;

            Ok(SeatEffect::new(move |label, board, rctx| {
                if let Answer::Ok(omen) = ans {
                    board.oracle_omen = omen;
                    rctx.record(irc_format!(
                        "🔮🛸 {label} ({}) tuned cosmic frequencies to {:?}.",
                        rctx.nick(seat).bold(),
                        omen,
                    ));
                }
            }))
        })
    }
}

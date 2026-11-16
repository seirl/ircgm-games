use ircgm::prelude::*;

use super::psychic::PSYCHIC;
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
            "Answer a Mother-Ship question at the start of Dusk/Night that alters cosmic visions and Alien-set powers!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(1, Self::prompt).mutates().follow_on_copy())
    }
}

impl Oracle {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let ans = ctx
                .ask(
                    irc_format!(
                        "🔮🛸 Oracle: The Mother Ship asks — tune cosmic frequencies to {} (boosts visions), {}, or {}?",
                        "amplify".bold(),
                        "static".bold(),
                        "normal".bold(),
                    ),
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
                        _ => AskMatch::reject("Reply amplify, static, or normal.".yellow()),
                    },
                )
                .await?;

            Ok(SeatEffect::new(move |label, board, rctx| {
                let (omen, timed_out) = match ans.ok() {
                    Some(o) => (o, false),
                    None => {
                        let choices = [
                            OracleOmen::AmplifiedSignals,
                            OracleOmen::QuantumStatic,
                            OracleOmen::Normal,
                        ];
                        let o = choices[fastrand::usize(..choices.len())];
                        (o, true)
                    }
                };
                board.oracle_omen = omen;
                if timed_out {
                    rctx.dm(
                        seat,
                        irc_format!(
                            "🔮🛸 Time's up — the Mother Ship randomly tuned cosmic frequencies to {:?}!",
                            omen,
                        ),
                    );
                }

                let actor_nick = rctx.nick(seat);
                if rctx.roster.deck_has(&PSYCHIC) {
                    rctx.record(irc_format!(
                        "🔮🛸 {label} ({}) tuned cosmic frequencies to {:?}.",
                        actor_nick.bold(),
                        omen,
                    ));
                } else {
                    match omen {
                        OracleOmen::AmplifiedSignals => {
                            board.record_card_interaction(seat);
                            let seen = board.center[0];
                            rctx.dm(
                                seat,
                                irc_format!(
                                    "🔮🛸 Oracle Vision: Center #1 holds {}!",
                                    seen.display_for_peek(),
                                ),
                            );
                            rctx.record(irc_format!(
                                "🔮🛸 {label} ({}) tuned frequencies to {:?} and foresaw Center #1 ({}).",
                                actor_nick.bold(),
                                omen,
                                seen.display_for_peek(),
                            ));
                        }
                        OracleOmen::QuantumStatic => {
                            board.record_card_interaction(seat);
                            board.center.swap(0, 2);
                            rctx.dm(
                                seat,
                                "🔮🛸 Quantum Static scrambled the cosmos — Center #1 and Center #3 swapped places!",
                            );
                            rctx.record(irc_format!(
                                "🔮🛸 {label} ({}) engaged {:?}, swapping Center #1 and Center #3.",
                                actor_nick.bold(),
                                omen,
                            ));
                        }
                        OracleOmen::Normal => {
                            if !board.is_shielded(seat) {
                                board.record_card_interaction(seat);
                                let seen = board.players[seat];
                                rctx.dm(
                                    seat,
                                    irc_format!(
                                        "🔮🛸 Oracle Reflection: Your own card is {}!",
                                        seen.display_for_peek(),
                                    ),
                                );
                            }
                            rctx.record(irc_format!(
                                "🔮🛸 {label} ({}) kept cosmic frequencies {:?}.",
                                actor_nick.bold(),
                                omen,
                            ));
                        }
                    }
                }
            }))
        })
    }
}

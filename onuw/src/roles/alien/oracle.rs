use ircgm::prelude::*;

use super::psychic::PSYCHIC;
use crate::board::{BoardAccess, BoardKey};
use crate::role::{GameSet, PromptCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum OracleOmen {
    #[default]
    Normal,
    AmplifiedSignals,
    QuantumStatic,
}

impl BoardKey for OracleOmen {
    const ACCESS: u16 = BoardAccess::CUSTOM_0;

    fn init() -> Self {
        Self::Normal
    }
}

pub struct Oracle;
pub static ORACLE: Oracle = Oracle;

impl Role for Oracle {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "oracle",
            "Oracle",
            GameSet::Alien,
            Team::Village,
            "Answer a Mother-Ship question at the start of Dusk/Night that alters cosmic frequencies and Alien-set powers!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&ORACLE, 1, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .writes(OracleOmen::ACCESS | BoardAccess::CENTER_FACES)]
        }
    }
}

impl Oracle {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
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
            board.set_key(omen);
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
                        let seen = board.peek_center(seat, 0);
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
                        board.swap_centers(seat, 0, 2);
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
                        if let Some(seen) = board.peek_player(seat, seat) {
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
    }
}

use ircgm::prelude::*;

use super::oracle::{OracleOmen, ORACLE};
use crate::board::{BoardKey, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Psychic;
pub static PSYCHIC: Psychic = Psychic;

impl Role for Psychic {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "psychic",
            "Psychic",
            GameSet::Alien,
            Team::Village,
            "Choose 1 neighbor's card to view (or view BOTH neighbors' cards if the Oracle amplified signals tonight!).",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has(&ORACLE) {
            self.info().summary.to_owned()
        } else {
            "Wake in Night to view either your left or right neighbor's card!".to_owned()
        }
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&PSYCHIC, 159, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .reads(OracleOmen::ACCESS)]
        }
    }
}

impl Psychic {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let omen = ctx.get_key::<OracleOmen>();
        let roster = ctx.roster;
        let view = ctx.view();
        let (left, right) = roster.neighbors(seat);

        let targets: Vec<usize> = if omen == OracleOmen::AmplifiedSignals {
            let mut v = vec![left, right];
            v.dedup();
            v
        } else {
            let unshielded: Vec<usize> = [left, right]
                .into_iter()
                .filter(|&s| !view.is_shielded(s))
                .collect();
            if unshielded.is_empty() {
                Vec::new()
            } else {
                let left_nick = roster.seats[left].user.nick();
                let right_nick = roster.seats[right].user.nick();
                let prompt = irc_format!(
                    "🧠 Psychic: Reply {} ({}) or {} ({}) to read a neighbor's mind!",
                    "left".bold(),
                    left_nick,
                    "right".bold(),
                    right_nick,
                );
                let ans = ctx
                    .ask(prompt, move |m| {
                        let raw = m.text.trim();
                        let lower = raw.to_ascii_lowercase();
                        let picked = match lower.as_str() {
                            "left" | "l" => Some(left),
                            "right" | "r" => Some(right),
                            _ => roster
                                .resolve_nick(raw)
                                .filter(|&i| i == left || i == right),
                        };
                        match picked {
                            Some(t) if view.is_shielded(t) => {
                                AskMatch::reject("🛡️ That neighbor's mind is shielded!".yellow())
                            }
                            Some(t) => AskMatch::accept_say(
                                t,
                                irc_format!(
                                    "🧠 Locked in reading {}'s mind — vision arrives when the phase ends!",
                                    roster.seats[t].user.nick().bold(),
                                ),
                            ),
                            None => AskMatch::reject("Reply left or right.".yellow()),
                        }
                    })
                    .await?;
                let fallback = match omen {
                    OracleOmen::QuantumStatic if !view.is_shielded(right) => right,
                    _ if !view.is_shielded(left) => left,
                    _ => unshielded[0],
                };
                vec![ans.ok().unwrap_or(fallback)]
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, &targets, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        targets: &[usize],
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        let mut unique_targets = targets.to_vec();
        unique_targets.dedup();

        let mut seen_parts = Vec::new();
        for target in unique_targets {
            if let Some(seen) = board.peek_player(seat, target) {
                let target_nick = ctx.nick(target);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🧠 Psychic Vision: {} holds {}!",
                        target_nick.as_str().bold(),
                        seen.display_for_peek(),
                    ),
                );
                seen_parts.push(irc_format!(
                    "{} ({})",
                    target_nick.bold(),
                    seen.display_for_peek()
                ));
            }
        }

        if seen_parts.is_empty() {
            ctx.dm(seat, "🧠 Psychic: Neighboring minds were shielded tonight!");
            ctx.record(irc_format!(
                "🧠 {actor_label} ({}) found neighboring cards shielded.",
                actor_nick.bold(),
            ));
        } else {
            ctx.record(irc_format!(
                "🧠 {actor_label} ({}) psychically read {}.",
                actor_nick.bold(),
                Text::join(seen_parts, ", "),
            ));
        }
    }
}

use ircgm::prelude::*;

use crate::board::{BoardMut, Roster, SeatSet};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};
use crate::roles::base::seer::SEER;
use crate::roles::daybreak::apprentice_seer::APPRENTICE_SEER;

pub struct Beholder;
pub static BEHOLDER: Beholder = Beholder;

impl Role for Beholder {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "beholder",
            "Beholder",
            GameSet::Vampire,
            Team::Village,
            "Wake near the end of Night to see who the Seer and Apprentice Seer are, and optionally check their current cards!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&BEHOLDER, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .copy_on_turn(),
            ]
        }
    }
}

impl Beholder {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let seers = (ctx.seats_with_role(&SEER) | ctx.seats_with_role(&APPRENTICE_SEER))
            .without(seat);
        if seers.is_empty() {
            ctx.say(irc_format!(
                "👁️🔮 Beholder: {}!",
                "NO player holds Seer or Apprentice Seer".bold().yellow(),
            ));
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "👁️🔮 {label} ({}) saw no Seer or Apprentice Seer in play.",
                    rctx.nick(seat).bold(),
                ));
            }));
        }

        let unshielded_seers = seers - ctx.shielded_seats();
        let seer_list = ctx.roster.format_seats(seers);
        if unshielded_seers.is_empty() {
            ctx.say(irc_format!(
                "👁️🔮 Beholder: Seer / Apprentice Seer thumbs up: {} — all shielded, so you cannot inspect their cards!",
                seer_list.as_str().bold().cyan(),
            ));
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "👁️🔮 {label} ({}) saw Seer/Apprentice Seer: {} (all shielded).",
                    rctx.nick(seat).bold(),
                    rctx.roster.format_seats(seers).bold().cyan(),
                ));
            }));
        }

        let view = ctx.view();
        let prompt = irc_format!(
            "👁️🔮 Beholder: Seer / Apprentice Seer thumbs up: {}! Reply {} to check all their current cards, {} for specific ones, or {}:",
            seer_list.as_str().bold().cyan(),
            "all".bold(),
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                let raw = m.text.trim();
                if Roster::is_pass(raw) {
                    return AskMatch::accept_say(
                        SeatSet::EMPTY,
                        "👁️🔮 Left Seer cards uninspected.",
                    );
                }
                if matches!(raw.to_ascii_lowercase().as_str(), "all" | "yes" | "y") {
                    return AskMatch::accept_say(
                        unshielded_seers,
                        "👁️🔮 Locked in checking Seer card(s) — reveals when the phase ends!",
                    );
                }
                let mut picked = SeatSet::EMPTY;
                for tok in raw.split_whitespace() {
                    let Some(idx) = view.resolve_nick(tok) else {
                        return AskMatch::reject(
                            format!("Unknown nickname '{tok}' (reply all, <nick>, or pass).")
                                .yellow(),
                        );
                    };
                    if !seers.contains(idx) {
                        return AskMatch::reject(
                            "You may only check players with Seer / Apprentice Seer thumbs up!"
                                .yellow(),
                        );
                    }
                    if view.is_shielded(idx) {
                        return AskMatch::reject("🛡️ That player's card is shielded!".yellow());
                    }
                    picked.insert(idx);
                }
                if picked.is_empty() {
                    return AskMatch::reject("Reply all, <nick>, or pass.".yellow());
                }
                AskMatch::accept_say(
                    picked,
                    "👁️🔮 Locked in checking Seer card(s) — reveals when the phase ends!",
                )
            })
            .await?;

        let to_peek = ans.ok().unwrap_or(SeatSet::EMPTY);
        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, seers, to_peek, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        seers: SeatSet,
        to_peek: SeatSet,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        let seer_names = ctx.roster.format_seats(seers);
        if to_peek.is_empty() {
            ctx.record(irc_format!(
                "👁️🔮 {actor_label} ({}) saw Seer/Apprentice Seer ({}) without checking cards.",
                actor_nick.bold(),
                seer_names.bold().cyan(),
            ));
            return;
        }

        let mut dm_parts = Vec::new();
        let mut chron_parts = Vec::new();
        for s in to_peek {
            if let Some(card) = board.peek_player(seat, s) {
                let s_nick = ctx.nick(s);
                dm_parts.push(irc_format!(
                    "{}: {}",
                    s_nick.as_str().bold(),
                    card.display_for_peek(),
                ));
                chron_parts.push(irc_format!(
                    "{} ({})",
                    s_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
        if !dm_parts.is_empty() {
            ctx.dm(
                seat,
                irc_format!(
                    "👁️🔮 Beholder Card Check — {}",
                    Text::join(&dm_parts, ", ".grey()),
                ),
            );
            ctx.record(irc_format!(
                "👁️🔮 {actor_label} ({}) saw Seer/Apprentice Seer ({}) and checked: {}.",
                actor_nick.bold(),
                seer_names.bold().cyan(),
                Text::join(&chron_parts, ", ".grey()),
            ));
        }
    }
}

#[cfg(test)]
#[path = "beholder_test.rs"]
mod test;

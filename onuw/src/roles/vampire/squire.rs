use ircgm::prelude::*;

use crate::board::{BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

pub struct Squire;
pub static SQUIRE: Squire = Squire;

impl Role for Squire {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "squire",
            "Squire",
            GameSet::Vampire,
            Team::Werewolf,
            "On the Werewolf team (dying does not lose for Werewolves). Wake near the end of Night to see who the Werewolves are and optionally check their current cards!",
        )
        .minion()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&SQUIRE, |s, ctx| Box::pin(Self::prompt(s, ctx))).copy_on_turn()]
        }
    }
}

impl Squire {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let wolves = ctx.seats_with_faction(NightFaction::Werewolf).without(seat);
        if wolves.is_empty() {
            ctx.say(irc_format!(
                "🛡️🐺 Squire: {}! Survive and get another player killed to win!",
                "NO players hold Werewolf roles".bold().yellow(),
            ));
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "🛡️🐺 {label} ({}) saw no Werewolves in play.",
                    rctx.nick(seat).bold(),
                ));
            }));
        }

        let unshielded_wolves = wolves - ctx.shielded_seats();
        let wolf_list = ctx.roster.format_seats(wolves);
        if unshielded_wolves.is_empty() {
            ctx.say(irc_format!(
                "🛡️🐺 Squire: Werewolf thumbs up: {} — all shielded, so you cannot inspect their cards!",
                wolf_list.as_str().bold().red(),
            ));
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "🛡️🐺 {label} ({}) saw Werewolf pack: {} (all shielded).",
                    rctx.nick(seat).bold(),
                    rctx.roster.format_seats(wolves).bold().red(),
                ));
            }));
        }

        let view = ctx.view();
        let prompt = irc_format!(
            "🛡️🐺 Squire: Werewolf thumbs up: {}! Reply {} to check all their current cards, {} for specific Werewolves, or {}:",
            wolf_list.as_str().bold().red(),
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
                        "🛡️🐺 Left Werewolf cards uninspected.",
                    );
                }
                if matches!(raw.to_ascii_lowercase().as_str(), "all" | "yes" | "y") {
                    return AskMatch::accept_say(
                        unshielded_wolves,
                        "🛡️🐺 Locked in checking Werewolf cards — reveals when the phase ends!",
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
                    if !wolves.contains(idx) {
                        return AskMatch::reject(
                            "You may only check players with Werewolf thumbs up!".yellow(),
                        );
                    }
                    if view.is_shielded(idx) {
                        return AskMatch::reject("🛡️ That Werewolf's card is shielded!".yellow());
                    }
                    picked.insert(idx);
                }
                if picked.is_empty() {
                    return AskMatch::reject("Reply all, <nick>, or pass.".yellow());
                }
                AskMatch::accept_say(
                    picked,
                    "🛡️🐺 Locked in checking Werewolf card(s) — reveals when the phase ends!",
                )
            })
            .await?;

        let to_peek = ans.ok().unwrap_or(SeatSet::EMPTY);
        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, wolves, to_peek, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        wolves: SeatSet,
        to_peek: SeatSet,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        let wolf_names = ctx.roster.format_seats(wolves);
        if to_peek.is_empty() {
            ctx.record(irc_format!(
                "🛡️🐺 {actor_label} ({}) saw Werewolves ({}) without checking cards.",
                actor_nick.bold(),
                wolf_names.bold().red(),
            ));
            return;
        }

        let mut dm_parts = Vec::new();
        let mut chron_parts = Vec::new();
        for w in to_peek {
            if let Some(card) = board.peek_player(seat, w) {
                let w_nick = ctx.nick(w);
                dm_parts.push(irc_format!(
                    "{}: {}",
                    w_nick.as_str().bold(),
                    card.display_for_peek(),
                ));
                chron_parts.push(irc_format!(
                    "{} ({})",
                    w_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
        if !dm_parts.is_empty() {
            ctx.dm(
                seat,
                irc_format!(
                    "🛡️🐺 Squire Card Check — {}",
                    Text::join(&dm_parts, ", ".grey()),
                ),
            );
            ctx.record(irc_format!(
                "🛡️🐺 {actor_label} ({}) saw Werewolves ({}) and checked: {}.",
                actor_nick.bold(),
                wolf_names.bold().red(),
                Text::join(&chron_parts, ", ".grey()),
            ));
        }
    }
}

#[cfg(test)]
#[path = "squire_test.rs"]
mod test;

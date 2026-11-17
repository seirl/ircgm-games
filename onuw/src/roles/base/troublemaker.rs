use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Troublemaker;
pub static TROUBLEMAKER: Troublemaker = Troublemaker;

impl Role for Troublemaker {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "troublemaker",
            "Troublemaker",
            GameSet::Base,
            Team::Village,
            "Wake to optionally swap two OTHER players' cards without looking at them.",
        )
        .aliases(&["tm", "trouble"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&TROUBLEMAKER, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .writes(BoardAccess::PLAYER_FACES),
            ]
        }
    }
}

impl Troublemaker {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "😈 Troublemaker: Reply with TWO other players ({}) to swap, or {}. Players: {players_str}",
            "<nick1> <nick2>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "😈 Passed — left all cards untouched.");
                }
                let parts: Vec<&str> = m.text.split_whitespace().collect();
                if parts.len() != 2 {
                    return AskMatch::reject("Usage: <nick1> <nick2> or pass.".yellow());
                }
                let Some(a) = view.resolve_nick(parts[0]) else {
                    return AskMatch::reject(format!("Unknown player '{}'.", parts[0]).yellow());
                };
                let Some(b) = view.resolve_nick(parts[1]) else {
                    return AskMatch::reject(format!("Unknown player '{}'.", parts[1]).yellow());
                };
                if a == seat || b == seat || a == b {
                    return AskMatch::reject(
                        "Pick TWO DIFFERENT players other than yourself!".yellow(),
                    );
                }
                if view.is_shielded(a) || view.is_shielded(b) {
                    return AskMatch::reject(
                        "🛡️ Shielded players cannot have their cards swapped!".yellow(),
                    );
                }
                AskMatch::accept_say(
                    Some((a, b)),
                    irc_format!(
                        "😈 Locked in swap between {} and {}!",
                        view.roster.seats[a].user.nick().bold(),
                        view.roster.seats[b].user.nick().bold(),
                    ),
                )
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<(usize, usize)>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if let Answer::Ok(Some((a, b))) = ans {
            if let Some((card_a, card_b)) = board.swap_players(seat, a, b) {
                ctx.record(irc_format!(
                    "😈 {actor_label} ({}) swapped {} ({}) and {} ({}).",
                    actor_nick.bold(),
                    ctx.nick(a).bold(),
                    card_a.display_for_peek(),
                    ctx.nick(b).bold(),
                    card_b.display_for_peek(),
                ));
            }
        } else {
            ctx.record(irc_format!(
                "😈 {actor_label} ({}) did not swap anyone.",
                actor_nick.bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "troublemaker_test.rs"]
mod test;

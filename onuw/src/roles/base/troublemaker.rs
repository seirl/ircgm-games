use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake};

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

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(170, |seat, ctx| Box::pin(Self::prompt(seat, ctx))).mutates())
    }
}

impl Troublemaker {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let (roster, shielded) = (ctx.roster, ctx.shielded);
        let prompt = irc_format!(
            "😈 Troublemaker: Reply with TWO other players ({}) to swap, or {}. Players: {}",
            "<nick1> <nick2>".bold(),
            "pass".bold(),
            roster.format_players_marking_shields(shielded),
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
                let Some(a) = roster.resolve_nick(parts[0]) else {
                    return AskMatch::reject(format!("Unknown player '{}'.", parts[0]).yellow());
                };
                let Some(b) = roster.resolve_nick(parts[1]) else {
                    return AskMatch::reject(format!("Unknown player '{}'.", parts[1]).yellow());
                };
                if a == seat || b == seat || a == b {
                    return AskMatch::reject(
                        "Pick TWO DIFFERENT players other than yourself!".yellow(),
                    );
                }
                if shielded.contains(&a) || shielded.contains(&b) {
                    return AskMatch::reject(
                        "🛡️ Shielded players cannot have their cards swapped!".yellow(),
                    );
                }
                AskMatch::accept_say(
                    Some((a, b)),
                    irc_format!(
                        "😈 Locked in swap between {} and {}!",
                        roster.seats[a].user.nick().bold(),
                        roster.seats[b].user.nick().bold(),
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
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if let Answer::Ok(Some((a, b))) = ans {
            if board.is_shielded(a) || board.is_shielded(b) {
                return;
            }
            board.record_card_interaction(seat);
            let card_a = board.players[a];
            let card_b = board.players[b];
            board.players.swap(a, b);
            ctx.record(irc_format!(
                "😈 {actor_label} ({}) swapped {} ({}) and {} ({}).",
                actor_nick.bold(),
                ctx.nick(a).bold(),
                card_a.display_for_peek(),
                ctx.nick(b).bold(),
                card_b.display_for_peek(),
            ));
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

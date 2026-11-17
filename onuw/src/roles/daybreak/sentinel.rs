use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Sentinel;
pub static SENTINEL: Sentinel = Sentinel;

impl Role for Sentinel {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "sentinel",
            "Sentinel",
            GameSet::Daybreak,
            Team::Village,
            "Wake first at Night (#0) to optionally place a Shield token on another player's card, preventing it from being viewed, moved, flipped, or given an Artifact.",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&SENTINEL, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .writes(BoardAccess::SHIELDS)]
        }
    }
}

impl Sentinel {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🛡️ Sentinel: Reply with {} to place a Shield token on another player's card, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🛡️ Passed — placed no Shield token.");
                }
                match view.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => AskMatch::reject(
                        "You cannot shield your own card! Pick another player or pass.".yellow(),
                    ),
                    Some(idx) if view.is_shielded(idx) => AskMatch::reject(
                        "🛡️ That player's card already has a Shield token!".yellow(),
                    ),
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🛡️ Placed a Shield token on {}'s card!",
                            view.roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Unknown player nickname (or pass).".yellow()),
                }
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<usize>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if let Answer::Ok(Some(target)) = ans {
            board.place_shield(target);
            ctx.record(irc_format!(
                "🛡️ {actor_label} ({}) placed a Shield token on {}'s card.",
                actor_nick.bold(),
                ctx.nick(target).bold(),
            ));
        } else {
            ctx.record(irc_format!(
                "🛡️ {actor_label} ({}) placed no Shield token.",
                actor_nick.bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "sentinel_test.rs"]
mod test;

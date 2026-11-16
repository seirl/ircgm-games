use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct RoleRetriever;
pub static ROLE_RETRIEVER: RoleRetriever = RoleRetriever;

impl Role for RoleRetriever {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "role_retriever",
            "Role Retriever",
            GameSet::SuperVillains,
            Team::Village,
            "May swap your card with another player's card and view your new card!",
        )
        .aliases(&["rr", "retriever"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&ROLE_RETRIEVER, 162, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .immediate_copy()
                    .writes(BoardAccess::PLAYER_FACES),
            ]
        }
    }
}

impl RoleRetriever {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        if ctx.is_shielded(seat) {
            ctx.say("🛡️ Your own card has a Shield token — you cannot swap it tonight!");
            return Ok(SeatEffect::noop());
        }

        let roster = ctx.roster;
        let view = ctx.view();
        let players_str = ctx.format_visible_players();
        let prompt = irc_format!(
            "🐕 Role Retriever: Reply {} to swap cards with another player and see your new card, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🐕 Passed — not swapping with anyone.");
                }
                match roster.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => {
                        AskMatch::reject("Pick ANOTHER player (or pass)!".yellow())
                    }
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ Shielded players cannot be swapped!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🐕 Locked in swapping with {} — new card reveals when the phase ends!",
                            roster.seats[idx].user.nick().bold(),
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
        if let Answer::Ok(Some(target)) = ans {
            let Some((victim_new, actor_new)) = board.swap_players(seat, seat, target) else {
                return;
            };
            let victim_nick = ctx.nick(target);

            ctx.dm(
                seat,
                irc_format!(
                    "🐕 Swapped with {} — you fetched {}!",
                    victim_nick.as_str().bold(),
                    actor_new.display_for_peek(),
                ),
            );
            ctx.record(irc_format!(
                "🐕 {actor_label} ({}) fetched {} from {} (giving {} {}).",
                ctx.nick(seat).bold(),
                actor_new.display_for_peek(),
                victim_nick.as_str().bold(),
                victim_nick.as_str().bold(),
                victim_new.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "role_retriever_test.rs"]
mod test;

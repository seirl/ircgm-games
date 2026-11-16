use ircgm::prelude::*;

use crate::board::{BoardMut, Roster};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};
use crate::roles::base::werewolf::WOLF_PACK_STEP;

pub struct MysticWolf;
pub static MYSTIC_WOLF: MysticWolf = MysticWolf;

impl Role for MysticWolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mystic_wolf",
            "Mystic Wolf",
            GameSet::Daybreak,
            Team::Werewolf,
            "Wake with the Werewolves, then look at 1 other player's card!",
        )
        .aliases(&["mw", "mystic"])
        .night_faction(NightFaction::Werewolf)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WOLF_PACK_STEP,
                WakeStep::solo(&MYSTIC_WOLF, 123, |s, ctx| Box::pin(Self::prompt(s, ctx))),
            ]
        }
    }
}

impl MysticWolf {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.format_visible_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🐺🔮 Mystic Wolf: Reply {} to inspect another player's card, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🐺🔮 Passed — not inspecting anyone.");
                }
                match view.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => {
                        AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                    }
                    Some(idx) if view.is_shielded(idx) => AskMatch::reject(
                        "🛡️ That player's card is shielded by the Sentinel!".yellow(),
                    ),
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🐺🔮 Locked in inspecting {} — card reveals when the phase ends!",
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
        if let Answer::Ok(Some(target)) = ans {
            if let Some(card) = board.peek_player(seat, target) {
                let target_nick = ctx.nick(target);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🐺🔮 {}'s card is: {}",
                        target_nick.as_str().bold(),
                        card.display_for_peek()
                    ),
                );
                ctx.record(irc_format!(
                    "🐺🔮 {actor_label} ({}) viewed {}'s card ({}).",
                    ctx.nick(seat).bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
    }
}

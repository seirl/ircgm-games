use ircgm::prelude::*;

use super::henchman::VILLAIN_PACK_STEP;
use crate::board::{BoardMut, Roster};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

pub struct DrPeeker;
pub static DR_PEEKER: DrPeeker = DrPeeker;

impl Role for DrPeeker {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "dr_peeker",
            "Dr. Peeker",
            GameSet::SuperVillains,
            Team::SuperVillain,
            "Wake with the Super Villains, then may inspect 1 other player's card!",
        )
        .aliases(&["peeker", "drpeeker"])
        .night_faction(NightFaction::SuperVillain)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                VILLAIN_PACK_STEP,
                WakeStep::solo(&DR_PEEKER, 126, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .copy_on_turn(),
            ]
        }
    }
}

impl DrPeeker {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let view = ctx.view();
        let valid_targets = roster.all_seats().without(seat) - view.shielded_seats();

        if valid_targets.is_empty() {
            ctx.say(
                "🛡️ Dr. Peeker: Every other player's card is shielded — you cannot X-ray anyone tonight!"
                    .yellow(),
            );
            return Ok(SeatEffect::noop());
        }

        let players_str = ctx.roster.format_players();
        let prompt = irc_format!(
            "🔬 Dr. Peeker: Reply {} to X-ray another player's card, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🔬 Passed — not X-raying anyone.");
                }
                match roster.resolve_nick(&m.text) {
                    Some(idx) if valid_targets.contains(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🔬 Locked in X-raying {} — result arrives when the phase ends!",
                            roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    Some(idx) if idx == seat => {
                        AskMatch::reject("Pick ANOTHER player (or pass)!".yellow())
                    }
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ That player is shielded!".yellow())
                    }
                    Some(_) => AskMatch::reject("Pick another player!".yellow()),
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
            let Some(seen) = board.peek_player(seat, target) else {
                return;
            };
            let target_nick = ctx.nick(target);
            ctx.dm(
                seat,
                irc_format!(
                    "🔬 X-Ray Result: {} holds {}!",
                    target_nick.as_str().bold(),
                    seen.display_for_peek(),
                ),
            );
            ctx.record(irc_format!(
                "🔬 {actor_label} ({}) X-rayed {} and saw {}.",
                ctx.nick(seat).bold(),
                target_nick.bold(),
                seen.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "dr_peeker_test.rs"]
mod test;

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
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
        .tags(&[RoleTag::VillainPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(126, Self::prompt).follow_on_copy())
    }
}

impl DrPeeker {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let valid_targets: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !ctx.shielded.contains(&i))
                .collect();

            if valid_targets.is_empty() {
                return Ok(SeatEffect::noop());
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = irc_format!(
                "🔬 Dr. Peeker: Reply {} to X-ray another player's card, or {}. Players: {}",
                "<nick>".bold(),
                "pass".bold(),
                roster.format_players_marking_shields(shielded),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if valid_targets.contains(&idx) => AskMatch::accept(Some(idx)),
                        Some(idx) if idx == seat => {
                            AskMatch::reject("Pick ANOTHER player (or pass)!".yellow())
                        }
                        Some(idx) if shielded.contains(&idx) => {
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
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<usize>>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(target)) = ans {
            if board.is_shielded(target) {
                return;
            }
            board.record_card_interaction(seat);
            let seen = board.players[target];
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

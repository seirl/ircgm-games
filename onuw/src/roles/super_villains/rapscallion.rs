use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct Rapscallion;
pub static RAPSCALLION: Rapscallion = Rapscallion;

impl Role for Rapscallion {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "rapscallion",
            "Rapscallion",
            GameSet::SuperVillains,
            Team::Werewolf,
            "Wake with the Super Villains, then MUST swap the extra Villain/Evil card onto a non-Villain player!",
        )
        .tags(&[RoleTag::VillainPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(129, Self::prompt).mutates())
    }
}

impl Rapscallion {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let villains = ctx.seats_with_tag(RoleTag::VillainPack);
            let valid_targets: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !ctx.shielded.contains(&i) && !villains.contains(&i))
                .collect();

            if valid_targets.is_empty() {
                return Ok(SeatEffect::noop());
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = irc_format!(
                "🧨 Rapscallion: You MUST plant the extra Villain card on a non-Villain player ({})! Players: {}",
                "<nick>".bold(),
                roster.format_players_marking_shields(shielded),
            );
            let valid_for_match = valid_targets.clone();
            let ans = ctx
                .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                    Some(idx) if valid_for_match.contains(&idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🧨 Recruited {} into Villainy!",
                            roster.seats[idx].user.nick().bold().red(),
                        ),
                    ),
                    Some(idx) if shielded.contains(&idx) => {
                        AskMatch::reject("🛡️ That player is shielded!".yellow())
                    }
                    Some(_) => AskMatch::reject("Pick a non-Villain player!".yellow()),
                    None => AskMatch::reject("Unknown player nickname.".yellow()),
                })
                .await?;

            let chosen = ans
                .ok()
                .unwrap_or_else(|| valid_targets[fastrand::usize(..valid_targets.len())]);

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, chosen, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        target: usize,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if !board.is_shielded(target) {
            board.record_card_interaction(seat);
            let old_card = board.players[target];
            let planted = board.extra_evil_card;
            std::mem::swap(&mut board.players[target], &mut board.extra_evil_card);
            ctx.record(irc_format!(
                "🧨 {actor_label} ({}) planted {} onto {} (replacing {}).",
                ctx.nick(seat).bold(),
                planted.display_for_peek(),
                ctx.nick(target).bold(),
                old_card.display_for_peek(),
            ));
        }
    }
}

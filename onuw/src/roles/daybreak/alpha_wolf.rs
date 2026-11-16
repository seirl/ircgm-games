use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::role::{
    GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct AlphaWolf;
pub static ALPHA_WOLF: AlphaWolf = AlphaWolf;

impl Role for AlphaWolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "alpha_wolf",
            "Alpha Wolf",
            GameSet::Daybreak,
            Team::Werewolf,
            "Wake with the Werewolves, then MUST swap the center Wolf card with another non-Werewolf player's card!",
        )
        .aliases(&["aw", "alpha"])
        .tags(&[RoleTag::WolfPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(
            Wake::solo(122, |seat, ctx| Box::pin(Self::prompt(seat, ctx)))
                .mutates()
                .follow_on_copy(),
        )
    }
}

impl AlphaWolf {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let mut wolves = ctx.waking_seats_with_tag(RoleTag::WolfPack);
        for dw in ctx.waking_seats_with_tag(RoleTag::DreamWolfThumb) {
            if !wolves.contains(&dw) {
                wolves.push(dw);
            }
        }
        let non_wolves: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| i != seat && !wolves.contains(&i))
            .collect();
        let valid_targets: Vec<usize> = non_wolves
            .iter()
            .copied()
            .filter(|i| !ctx.shielded.contains(i))
            .collect();

        if valid_targets.is_empty() {
            ctx.say("🐺 Alpha Wolf: No valid non-Wolf targets exist tonight!");
            return Ok(SeatEffect::noop());
        }

        let (roster, shielded) = (ctx.roster, ctx.shielded);
        let prompt = irc_format!(
            "🐺 Alpha Wolf: You MUST swap the center Werewolf card onto a non-Wolf player ({})! Non-Wolves: {}",
            "<nick>".bold(),
            roster.format_indices_marking_shields(&non_wolves, shielded),
        );
        let valid_for_match = valid_targets.clone();
        let ans = ctx
            .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                Some(idx) if valid_for_match.contains(&idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🐺 Swapped the center Wolf card with {}'s card!",
                        roster.seats[idx].user.nick().bold().red()
                    ),
                ),
                Some(idx) if shielded.contains(&idx) => {
                    AskMatch::reject("🛡️ That player's card is shielded by the Sentinel!".yellow())
                }
                Some(_) => AskMatch::reject("Pick another non-Werewolf player!".yellow()),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;

        let chosen = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let auto = valid_targets[fastrand::usize(..valid_targets.len())];
                ctx.say(format!(
                    "🐺 Randomly swapped the center Wolf card with {}'s card!",
                    ctx.nick(auto)
                ));
                auto
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, chosen, board, rctx);
        }))
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
                "🐺 {actor_label} ({}) bit {}, swapping {} in for {}.",
                ctx.nick(seat).bold(),
                ctx.nick(target).bold(),
                planted.display_for_peek(),
                old_card.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "alpha_wolf_test.rs"]
mod test;

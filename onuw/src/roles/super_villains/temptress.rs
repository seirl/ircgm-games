use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct Temptress;
pub static TEMPTRESS: Temptress = Temptress;

impl Role for Temptress {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "temptress",
            "Temptress",
            GameSet::SuperVillains,
            Team::Werewolf,
            "Wake with the Super Villains, then optionally swap YOUR Temptress card with a non-Villain player's card without looking!",
        )
        .tags(&[RoleTag::VillainPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(128, Self::prompt).mutates())
    }
}

impl Temptress {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let villains = ctx.seats_with_tag(RoleTag::VillainPack);
            let non_villains: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !ctx.shielded.contains(&i) && !villains.contains(&i))
                .collect();

            if non_villains.is_empty() {
                ctx.say("💋 Temptress: No valid non-Villain targets available!");
                return Ok(SeatEffect::noop());
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = irc_format!(
                "💋 Temptress: Reply {} to swap your Temptress card with a non-Villain (without looking at your new card!), or {}. Players: {}",
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
                        Some(idx) if non_villains.contains(&idx) => AskMatch::accept_say(
                            Some(idx),
                            irc_format!(
                                "💋 Swapped your Temptress card with {}!",
                                roster.seats[idx].user.nick().bold(),
                            ),
                        ),
                        Some(idx) if shielded.contains(&idx) => {
                            AskMatch::reject("🛡️ That player is shielded!".yellow())
                        }
                        Some(_) => AskMatch::reject("Pick a non-Villain player!".yellow()),
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
            if board.is_shielded(seat) || board.is_shielded(target) {
                return;
            }
            board.record_card_interaction(seat);
            let old_self = board.players[seat];
            let old_target = board.players[target];
            board.players.swap(seat, target);
            ctx.record(irc_format!(
                "💋 {actor_label} ({}) lured {} into taking {} (taking {} unseen).",
                ctx.nick(seat).bold(),
                ctx.nick(target).bold(),
                old_self.display_for_peek(),
                old_target.display_for_peek(),
            ));
        }
    }
}

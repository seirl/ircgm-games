use ircgm::prelude::*;

use crate::board::BoardCards;
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
            Team::SuperVillain,
            "Wake with the Super Villains, then MUST swap the extra Villain card onto a non-Villain player!",
        )
        .tags(&[RoleTag::VillainPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(128, Self::prompt).mutates().follow_on_copy())
    }
}

impl Temptress {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let villains = ctx.waking_seats_with_tag(RoleTag::VillainPack);
            let non_villains: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !villains.contains(&i))
                .collect();
            let valid_targets: Vec<usize> = non_villains
                .iter()
                .copied()
                .filter(|i| !ctx.shielded.contains(i))
                .collect();

            if valid_targets.is_empty() {
                ctx.say("💋 Temptress: No valid non-Villain targets available!");
                return Ok(SeatEffect::noop());
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = irc_format!(
                "💋 Temptress: You MUST plant the extra Villain card on a non-Villain player ({})! Non-Villains: {}",
                "<nick>".bold(),
                roster.format_indices_marking_shields(&non_villains, shielded),
            );
            let valid_for_match = valid_targets.clone();
            let ans = ctx
                .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                    Some(idx) if valid_for_match.contains(&idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "💋 Lured {} into Villainy!",
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

            let (chosen, timed_out) = match ans.ok() {
                Some(idx) => (idx, false),
                None => {
                    let idx = valid_targets[fastrand::usize(..valid_targets.len())];
                    (idx, true)
                }
            };

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, chosen, timed_out, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        target: usize,
        timed_out: bool,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if !board.is_shielded(target) {
            board.record_card_interaction(seat);
            let old_card = board.players[target];
            let planted = board.extra_villain_card;
            std::mem::swap(&mut board.players[target], &mut board.extra_villain_card);
            if timed_out {
                ctx.dm(
                    seat,
                    irc_format!(
                        "💋 Time's up — automatically planted the extra Villain card onto {}!",
                        ctx.nick(target).bold(),
                    ),
                );
            }
            ctx.record(irc_format!(
                "💋 {actor_label} ({}) planted {} onto {} (replacing {}).",
                ctx.nick(seat).bold(),
                planted.display_for_peek(),
                ctx.nick(target).bold(),
                old_card.display_for_peek(),
            ));
        }
    }
}

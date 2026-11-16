use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::marks::Mark;
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct Count;
pub static COUNT: Count = Count;

impl Role for Count {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "count",
            "The Count",
            GameSet::Vampire,
            Team::Vampire,
            "Vampire who MUST place the Mark of Fear on a non-Vampire player in Dusk, preventing them from waking in Night!",
        )
        .aliases(&["count", "the count"])
        .tags(&[RoleTag::VampirePack, RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(30, Self::prompt).dusk().reads_board().mutates())
    }
}

impl Count {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let board = ctx.board();
            let vamp_seats: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| {
                    board.players[i]
                        .effective_role()
                        .tags()
                        .contains(&RoleTag::VampirePack)
                        || board.marks[i] == Mark::Vampire
                })
                .collect();
            let valid_targets: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !vamp_seats.contains(&i))
                .collect();
            if valid_targets.is_empty() {
                return Ok(SeatEffect::noop());
            }

            let roster = ctx.roster;
            let valid_for_match = valid_targets.clone();
            let prompt = irc_format!(
                "🦇 The Count: You MUST reply {} to place the Mark of Fear on a non-Vampire (suppressing their Night wake)! Players: {}",
                "<nick>".bold(),
                roster.format_players(),
            );
            let ans = ctx
                .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                    Some(idx) if valid_for_match.contains(&idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🦇 Placed {} on {}!",
                            Mark::Fear.styled(),
                            roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    Some(_) => AskMatch::reject("You cannot terrify a fellow Vampire!".yellow()),
                    None => AskMatch::reject("Unknown player nickname.".yellow()),
                })
                .await?;

            let target = ans
                .ok()
                .unwrap_or_else(|| valid_targets[fastrand::usize(..valid_targets.len())]);

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, target, board, rctx);
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
        board.marks[target] = Mark::Fear;
        ctx.record(irc_format!(
            "🦇 {actor_label} ({}) placed {} on {}.",
            ctx.nick(seat).bold(),
            Mark::Fear.styled(),
            ctx.nick(target).bold(),
        ));
    }
}

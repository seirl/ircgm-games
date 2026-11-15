use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
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
            "Count",
            GameSet::Vampire,
            Team::Vampire,
            "Vampire who places the Mark of Fear on a non-Vampire player in Dusk, preventing them from waking in Night!",
        )
        .aliases(&["the count"])
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

            let roster = ctx.roster;
            let prompt = irc_format!(
                "🦇 Count: Reply {} to place the Mark of Fear (suppressing their Night wake!), or {}. Players: {}",
                "<nick>".bold(),
                "pass".bold(),
                roster.format_players(),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat || vamp_seats.contains(&idx) => {
                            AskMatch::reject("You cannot terrify a fellow Vampire!".yellow())
                        }
                        Some(idx) => AskMatch::accept_say(
                            Some(idx),
                            irc_format!(
                                "🦇 Placed {} on {}!",
                                Mark::Fear.styled(),
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
            board.marks[target] = Mark::Fear;
            ctx.record(irc_format!(
                "🦇 {actor_label} ({}) placed {} on {}.",
                ctx.nick(seat).bold(),
                Mark::Fear.styled(),
                ctx.nick(target).bold(),
            ));
        }
    }
}

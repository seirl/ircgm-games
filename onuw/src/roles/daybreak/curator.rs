use ircgm::prelude::*;

use crate::artifacts::Artifact;
use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Curator;
pub static CURATOR: Curator = Curator;

impl Role for Curator {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "curator",
            "Curator",
            GameSet::Daybreak,
            Team::Village,
            "May place a random face-down Artifact token on any player's card (secretly altering their team/goal!).",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(194, Self::prompt).mutates().follow_on_copy())
    }
}

impl Curator {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = irc_format!(
                "🏺 Curator: Reply {} to bestow a random Artifact token (including yourself), or {}. Players: {}",
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
                        Some(idx) if shielded.contains(&idx) => AskMatch::reject(
                            "🛡️ Shielded players cannot receive Artifacts!".yellow(),
                        ),
                        Some(idx) => AskMatch::accept_say(
                            Some(idx),
                            irc_format!(
                                "🏺 Placed a mysterious Artifact on {}!",
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
            if !board.is_shielded(target) {
                let art = Artifact::random();
                board.artifacts.insert(target, art);
                ctx.dm(target, art.recipient_dm());
                ctx.record(irc_format!(
                    "🏺 {actor_label} ({}) placed {} on {}.",
                    ctx.nick(seat).bold(),
                    art.styled(),
                    ctx.nick(target).bold(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "curator_test.rs"]
mod test;

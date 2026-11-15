use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::marks::Mark;
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Priest;
pub static PRIEST: Priest = Priest;

impl Role for Priest {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "priest",
            "Priest",
            GameSet::Vampire,
            Team::Village,
            "Cleanse yourself AND optionally 1 other player with the Mark of Clarity (wiping any Dusk marks on them!).",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has_marks() {
            self.info().summary.to_owned()
        } else {
            "No Mark-placing roles are in this deck — everyone already has clarity tonight."
                .to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(40, Self::prompt).dusk().mutates())
    }
}

impl Priest {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let prompt = irc_format!(
                "✝️ Priest: You cleanse yourself; reply {} to ALSO cleanse another player with Mark of Clarity, or {}. Players: {}",
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
                        Some(idx) if idx == seat => AskMatch::reject(
                            "You already cleanse yourself automatically! Pick another player or pass."
                                .yellow(),
                        ),
                        Some(idx) => AskMatch::accept_say(
                            Some(idx),
                            irc_format!(
                                "✝️ Cleansed yourself and {} with the {}!",
                                roster.seats[idx].user.nick().bold(),
                                Mark::Clarity.styled(),
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
        board.marks[seat] = Mark::Clarity;
        let actor_nick = ctx.nick(seat);
        if let Answer::Ok(Some(other)) = ans {
            board.marks[other] = Mark::Clarity;
            ctx.record(irc_format!(
                "✝️ {actor_label} ({}) cleansed self and {} with {}.",
                actor_nick.bold(),
                ctx.nick(other).bold(),
                Mark::Clarity.styled(),
            ));
        } else {
            ctx.record(irc_format!(
                "✝️ {actor_label} ({}) cleansed self with {}.",
                actor_nick.bold(),
                Mark::Clarity.styled(),
            ));
        }
    }
}

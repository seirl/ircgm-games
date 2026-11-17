use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::marks::Mark;
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&PRIEST, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .dusk()
                .copy_on_turn()
                .writes(BoardAccess::MARKS)]
        }
    }
}

impl Priest {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        if !roster.deck_has_marks() {
            ctx.say("✝️ Priest: No Mark-placing roles are in this deck — everyone already has clarity tonight.");
            return Ok(SeatEffect::noop());
        }
        let players_str = ctx.roster.format_players();
        let prompt = irc_format!(
            "✝️ Priest: You cleanse yourself; reply {} to ALSO cleanse another player with Mark of Clarity, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(
                        None,
                        irc_format!("✝️ Cleansed yourself with the {}!", Mark::Clarity.styled()),
                    );
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
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<usize>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        board.set_mark(seat, Mark::Clarity);
        let actor_nick = ctx.nick(seat);
        if let Answer::Ok(Some(other)) = ans {
            board.set_mark(other, Mark::Clarity);
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

#[cfg(test)]
#[path = "priest_test.rs"]
mod test;

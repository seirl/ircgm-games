use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Curator;
pub static CURATOR: Curator = Curator;

impl Role for Curator {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "curator",
            "Curator",
            GameSet::Daybreak,
            Team::Village,
            "May place a random face-down Artifact token on any player's card (secretly altering their role, team, or goal!).",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&CURATOR, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .copy_on_turn()
                .writes(BoardAccess::ARTIFACTS)]
        }
    }
}

impl Curator {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🏺 Curator: Reply {} to bestow a random Artifact token (including yourself), or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🏺 Passed — placed no Artifact.");
                }
                match view.resolve_nick(&m.text) {
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ Shielded players cannot receive Artifacts!".yellow())
                    }
                    // Rule (Daybreak — Curator): "The Doppelgänger-Curator
                    // wakes up after the Curator, and may place an Artifact
                    // token on any player's card that doesn't already have an
                    // Artifact or Shield token on it."
                    Some(idx) if view.has_artifact(idx) => {
                        AskMatch::reject("🏺 That player already has an Artifact token!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🏺 Placed a mysterious Artifact on {}!",
                            view.roster.seats[idx].user.nick().bold(),
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
        if let Answer::Ok(Some(target)) = ans {
            if let Some(art) = board.place_random_artifact(target, &ctx.roster.deck) {
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

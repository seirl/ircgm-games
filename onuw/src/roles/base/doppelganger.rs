//! Base ONUW **Doppelgänger** — polymorphic over any other [`Role`].

use ircgm::prelude::*;

use super::villager::VILLAGER;
use crate::board::BoardCards;
use crate::role::{
    BoxFuture, CopiedRecord, CopyBehavior, CopycatEffect, GameSet, PromptCtx, ResolveCtx, Role,
    RoleInfo, Team, Wake,
};

pub struct Doppelganger;
pub static DOPPELGANGER: Doppelganger = Doppelganger;

impl Role for Doppelganger {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "doppelganger",
            "Doppelgänger",
            GameSet::Base,
            Team::Village,
            "Wake 1st: look at another player's card and permanently become that role & team.",
        )
        .aliases(&["dg", "doppel", "doppelgänger"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::copycat(10, &VILLAGER, Self::pick))
    }
}

impl Doppelganger {
    fn pick<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<CopycatEffect>> {
        Box::pin(async move {
            let candidates: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !ctx.shielded.contains(&i))
                .collect();
            if candidates.is_empty() {
                return Ok(CopycatEffect::new(|_, _| None));
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let players_str = roster.format_players_marking_shields(shielded);
            let ans = ctx
                .ask(
                    format!(
                        "🎭 Doppelgänger: Pick ONE other player (`<nick>`) to copy! Players: {players_str}"
                    ),
                    move |m| match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat => {
                            AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                        }
                        Some(idx) if shielded.contains(&idx) => AskMatch::reject(
                            "🛡️ That player's card is shielded by the Sentinel!".yellow(),
                        ),
                        Some(idx) => AskMatch::accept(idx),
                        None => AskMatch::reject("Unknown player nickname.".yellow()),
                    },
                )
                .await?;

            Ok(CopycatEffect::new(move |board, rctx| {
                Self::resolve(seat, ans.ok(), board, rctx)
            }))
        })
    }

    fn resolve(
        seat: usize,
        chosen: Option<usize>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) -> Option<CopiedRecord> {
        let candidates: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| i != seat && !board.is_shielded(i))
            .collect();
        if candidates.is_empty() {
            return None;
        }

        let (target_idx, was_random) = match chosen.filter(|idx| candidates.contains(idx)) {
            Some(idx) => (idx, false),
            None => (candidates[fastrand::usize(..candidates.len())], true),
        };

        let copied = board.players[target_idx].effective_role();
        board.record_card_interaction(seat);
        board.players[seat].transform(copied);

        let actor_nick = ctx.nick(seat);
        let target_nick = ctx.nick(target_idx);
        let self_styled = DOPPELGANGER.info().styled_name();

        if was_random {
            ctx.dm(seat, format!("🎲 Randomly looked at {target_nick}."));
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) randomly copied {} ({}).",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                copied.styled_name(),
            ));
        } else {
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) copied {} and became {}.",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                copied.styled_name(),
            ));
        }

        ctx.dm(
            seat,
            irc_format!(
                "🎭 {}'s card is {}! You are now that role.",
                target_nick.bold(),
                copied.styled_name(),
            ),
        );
        match copied.copy_behavior() {
            CopyBehavior::ActImmediately => {}
            CopyBehavior::JoinGroupWake | CopyBehavior::WakeAfterAllMutations => {
                ctx.dm(
                    seat,
                    "🌙 Stand by—your copied role's night info will arrive on schedule!",
                );
            }
            CopyBehavior::Sleep => {
                ctx.dm(seat, "🌙 You have no further night actions.");
            }
        }

        Some(CopiedRecord {
            copycat_seat: seat,
            copied_role: copied,
        })
    }
}

#[cfg(test)]
#[path = "doppelganger_test.rs"]
mod test;

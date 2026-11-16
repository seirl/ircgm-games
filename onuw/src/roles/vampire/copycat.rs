use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct Copycat;
pub static COPYCAT: Copycat = Copycat;

impl Role for Copycat {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "copycat",
            "Copycat",
            GameSet::Vampire,
            Team::Village,
            "Wake first in Dusk: view 1 center card (1..=3) and become that role & team for the rest of the game!",
        )
        .aliases(&["cc"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(
            Wake::solo(8, |seat, ctx| Box::pin(Self::prompt(seat, ctx)))
                .dusk()
                .mutates()
                .reads_board()
                .short_budget(),
        )
    }
}

impl Copycat {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let ans = ctx
            .ask(
                irc_format!(
                    "🪞 Copycat: Pick a center card ({}, {}, or {}) to view and become!",
                    "1".bold(),
                    "2".bold(),
                    "3".bold(),
                ),
                |m| match Roster::parse_center_slot(&m.text) {
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        format!(
                            "🪞 Locked in Center #{} — role reveals when the phase ends!",
                            idx + 1
                        ),
                    ),
                    None => AskMatch::reject("Reply 1, 2, or 3.".yellow()),
                },
            )
            .await?;

        Ok(SeatEffect::new(move |_label, board, rctx| {
            Self::resolve(seat, ans.ok(), board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        chosen: Option<usize>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let (c_idx, was_random) = match chosen {
            Some(idx) => (idx, false),
            None => (fastrand::usize(0..3), true),
        };

        let copied = board.center[c_idx].effective_role();
        board.record_card_interaction(seat);
        ctx.become_role(seat, copied, format!("Copycat-{}", copied.name()), board);

        let actor_nick = ctx.nick(seat);
        let self_styled = COPYCAT.info().styled_name();

        if was_random {
            ctx.dm(seat, format!("🎲 Randomly peeked Center #{}.", c_idx + 1));
            ctx.record(irc_format!(
                "🪞 {self_styled} ({}) randomly copied Center #{} ({}).",
                actor_nick.bold(),
                c_idx + 1,
                copied.styled_name(),
            ));
        } else {
            ctx.record(irc_format!(
                "🪞 {self_styled} ({}) copied Center #{} and became {}.",
                actor_nick.bold(),
                c_idx + 1,
                copied.styled_name(),
            ));
        }

        ctx.dm(
            seat,
            irc_format!(
                "🪞 Center #{} is {}! You are now that role.",
                c_idx + 1,
                copied.styled_name(),
            ),
        );
        let wakes_later = copied.wake().is_some_and(|w| w.order > 8)
            || copied.tags().iter().any(|t| {
                matches!(
                    t,
                    RoleTag::WolfPack
                        | RoleTag::VampirePack
                        | RoleTag::AlienPack
                        | RoleTag::VillainPack
                )
            });
        if wakes_later {
            ctx.dm(
                seat,
                "🌙 Stand by—your copied role's action will fire on schedule!",
            );
        } else {
            ctx.dm(seat, "🌙 You have no further actions tonight.");
        }
    }
}

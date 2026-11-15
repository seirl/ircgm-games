use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, CopiedRecord, CopyBehavior, CopycatEffect, GameSet, PromptCtx, ResolveCtx, Role,
    RoleInfo, Team, Wake,
};
use crate::roles::base::villager::VILLAGER;

pub struct Copycat;
pub static COPYCAT: Copycat = Copycat;

impl Role for Copycat {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "copycat",
            "Copycat",
            GameSet::Vampire,
            Team::Independent,
            "Wake first in Dusk: view 1 center card (`1..=3`) and become that role & team for the rest of the game!",
        )
        .aliases(&["cc"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::copycat(5, &VILLAGER, Self::pick))
    }
}

impl Copycat {
    fn pick<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<CopycatEffect>> {
        Box::pin(async move {
            let ans = ctx
                .ask(
                    "🪞 Copycat: Pick a center card (`1`, `2`, or `3`) to view and become!",
                    |m| match Roster::parse_center_slot(&m.text) {
                        Some(idx) => AskMatch::accept(idx),
                        None => AskMatch::reject("Reply `1`, `2`, or `3`.".yellow()),
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
        let (c_idx, was_random) = match chosen {
            Some(idx) => (idx, false),
            None => (fastrand::usize(0..3), true),
        };

        let copied = board.center[c_idx].effective_role();
        board.record_card_interaction(seat);
        board.players[seat].transform(copied);

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
        match copied.copy_behavior() {
            CopyBehavior::ActImmediately => {}
            CopyBehavior::JoinGroupWake | CopyBehavior::WakeAfterAllMutations => {
                ctx.dm(
                    seat,
                    "🌙 Stand by—your copied role's action will fire on schedule!",
                );
            }
            CopyBehavior::Sleep => {
                ctx.dm(seat, "🌙 You have no further actions tonight.");
            }
        }

        Some(CopiedRecord {
            copycat_seat: seat,
            copied_role: copied,
        })
    }
}

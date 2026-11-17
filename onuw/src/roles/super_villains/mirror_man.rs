use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct MirrorMan;
pub static MIRROR_MAN: MirrorMan = MirrorMan;

impl Role for MirrorMan {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mirror_man",
            "Mirror Man",
            GameSet::SuperVillains,
            Team::Village,
            "Wake at the start of the night: view 1 center card (1..=3) and become that role & team for the rest of the game!",
        )
        .aliases(&["mirrorman", "mirror"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&MIRROR_MAN, 9, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .short_budget()
                    .copy_on_turn()
                    .writes(BoardAccess::PLAYER_BECAME | BoardAccess::WAKING_ROLES),
            ]
        }
    }
}

impl MirrorMan {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let ans = ctx
            .ask(
                irc_format!(
                    "🪞🦸 Mirror Man: Pick a center card ({}, {}, or {}) to view and become!",
                    "1".bold(),
                    "2".bold(),
                    "3".bold(),
                ),
                |m| match Roster::parse_center_slot(&m.text) {
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        format!(
                            "🪞🦸 Locked in Center #{} — role reveals when the phase ends!",
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
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let (c_idx, was_random) = match chosen {
            Some(idx) => (idx, false),
            None => (fastrand::usize(0..3), true),
        };

        let copied = board.copy_from_center(seat, c_idx, ctx);
        let actor_nick = ctx.nick(seat);
        let self_styled = MIRROR_MAN.info().styled_name();

        if was_random {
            ctx.dm(seat, format!("🎲 Randomly peeked Center #{}.", c_idx + 1));
            ctx.record(irc_format!(
                "🪞🦸 {self_styled} ({}) randomly copied Center #{} ({}).",
                actor_nick.bold(),
                c_idx + 1,
                copied.styled_name(),
            ));
        } else {
            ctx.record(irc_format!(
                "🪞🦸 {self_styled} ({}) copied Center #{} and became {}.",
                actor_nick.bold(),
                c_idx + 1,
                copied.styled_name(),
            ));
        }

        ctx.dm(
            seat,
            irc_format!(
                "🪞🦸 Center #{} is {}! You are now that role.",
                c_idx + 1,
                copied.styled_name(),
            ),
        );
        let wakes_later =
            !copied.is_doppelganger() && copied.steps().iter().any(|st| st.order > 9);
        if wakes_later {
            ctx.dm(
                seat,
                "🌙 Stand by — your copied role's action will fire on schedule!",
            );
        } else {
            ctx.dm(seat, "🌙 You have no further actions tonight.");
        }
    }
}

#[cfg(test)]
#[path = "mirror_man_test.rs"]
mod test;

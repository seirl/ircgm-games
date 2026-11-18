use ircgm::prelude::*;

use crate::board::{BoardMut, CenterSlot};
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
            "Wake at the start of the night: view 1 center card and become that role & team for the rest of the game!",
        )
        .aliases(&["mirrorman", "mirror"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::center_copier(&MIRROR_MAN, |s, ctx| {
                Box::pin(Self::prompt(s, ctx))
            })]
        }
    }
}

impl MirrorMan {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let hint = roster.center_choice_hint();
        let ans = ctx
            .ask(
                irc_format!(
                    "🪞🦸 Mirror Man: Pick a center card ({}) to view and become!",
                    hint.as_str().bold(),
                ),
                move |m| match roster.parse_center_slot(&m.text) {
                    Some(slot) => AskMatch::accept_say(
                        slot,
                        format!(
                            "🪞🦸 Locked in {} — role reveals when the phase ends!",
                            slot.label(roster)
                        ),
                    ),
                    None => AskMatch::reject(format!("Reply {hint}.").yellow()),
                },
            )
            .await?;

        Ok(SeatEffect::new(move |_label, board, rctx| {
            Self::resolve(seat, ans.ok(), board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        chosen: Option<CenterSlot>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let (slot, was_random) = match chosen {
            Some(s) => (s, false),
            None => (ctx.roster.choose_random_center_slot(), true),
        };

        let copied = board.copy_from_center(seat, slot, ctx);
        let slot_label = slot.label(ctx.roster);
        let actor_nick = ctx.nick(seat);
        let self_styled = MIRROR_MAN.info().styled_name();

        if was_random {
            ctx.dm(seat, format!("🎲 Randomly peeked {slot_label}."));
            ctx.record(irc_format!(
                "🪞🦸 {self_styled} ({}) randomly copied {slot_label} ({}).",
                actor_nick.bold(),
                copied.styled_name(),
            ));
        } else {
            ctx.record(irc_format!(
                "🪞🦸 {self_styled} ({}) copied {slot_label} and became {}.",
                actor_nick.bold(),
                copied.styled_name(),
            ));
        }

        ctx.dm(
            seat,
            irc_format!(
                "🪞🦸 {slot_label} is {}! You are now that role.",
                copied.styled_name(),
            ),
        );
        if crate::roles::wakes_after_center_copy(&MIRROR_MAN, copied) {
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

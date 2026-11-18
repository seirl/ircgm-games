use ircgm::prelude::*;

use super::henchman::VILLAIN_PACK_STEP;
use crate::board::{BoardMut, CenterSlot, Roster};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

pub struct Rapscallion;
pub static RAPSCALLION: Rapscallion = Rapscallion;

impl Role for Rapscallion {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "rapscallion",
            "Rapscallion",
            GameSet::SuperVillains,
            Team::SuperVillain,
            "Wake with the Super Villains, then may view 1 center card!",
        )
        .night_faction(NightFaction::SuperVillain)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                VILLAIN_PACK_STEP,
                WakeStep::solo(&RAPSCALLION, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .copy_on_turn(),
            ]
        }
    }
}

impl Rapscallion {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let hint = roster.center_or_pass_hint();
        let prompt = irc_format!(
            "🧨 Rapscallion: Reply {} to peek at one center card!",
            hint.as_str().bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🧨 Passed on peeking the center.");
                }
                match roster.parse_center_slot(&m.text) {
                    Some(slot) => AskMatch::accept_say(
                        Some(slot),
                        format!(
                            "🧨 Locked in {} — card reveals when the phase ends!",
                            slot.label(roster)
                        ),
                    ),
                    None => AskMatch::reject(format!("Reply {hint}.").yellow()),
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
        ans: Answer<Option<CenterSlot>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(slot)) = ans {
            let seen = board.peek_center(seat, slot);
            let slot_label = slot.label(ctx.roster);
            ctx.dm(
                seat,
                irc_format!(
                    "🧨 Rapscallion Peek: {slot_label} is {}!",
                    seen.display_for_peek(),
                ),
            );
            ctx.record(irc_format!(
                "🧨 {actor_label} ({}) peeked {slot_label} ({}).",
                ctx.nick(seat).bold(),
                seen.display_for_peek(),
            ));
        } else {
            ctx.record(irc_format!(
                "🧨 {actor_label} ({}) did not peek the center.",
                ctx.nick(seat).bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "rapscallion_test.rs"]
mod test;

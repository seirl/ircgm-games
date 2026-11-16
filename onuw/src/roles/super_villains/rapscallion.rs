use ircgm::prelude::*;

use super::henchman::VILLAIN_PACK_STEP;
use crate::board::{BoardMut, Roster};
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
                WakeStep::solo(&RAPSCALLION, 127, |s, ctx| Box::pin(Self::prompt(s, ctx))),
            ]
        }
    }
}

impl Rapscallion {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let prompt = irc_format!(
            "🧨 Rapscallion: Reply {}, {}, {}, or {} to peek at one center card!",
            "1".bold(),
            "2".bold(),
            "3".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🧨 Passed on peeking the center.");
                }
                match Roster::parse_center_slot(&m.text) {
                    Some(c_idx) => AskMatch::accept_say(
                        Some(c_idx),
                        format!(
                            "🧨 Locked in Center #{} — card reveals when the phase ends!",
                            c_idx + 1
                        ),
                    ),
                    None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
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
        if let Answer::Ok(Some(c_idx)) = ans {
            let seen = board.peek_center(seat, c_idx);
            ctx.dm(
                seat,
                irc_format!(
                    "🧨 Rapscallion Peek: Center #{} is {}!",
                    c_idx + 1,
                    seen.display_for_peek(),
                ),
            );
            ctx.record(irc_format!(
                "🧨 {actor_label} ({}) peeked Center #{} ({}).",
                ctx.nick(seat).bold(),
                c_idx + 1,
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

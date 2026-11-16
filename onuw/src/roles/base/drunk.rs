use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Drunk;
pub static DRUNK: Drunk = Drunk;

impl Role for Drunk {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "drunk",
            "Drunk",
            GameSet::Base,
            Team::Village,
            "Wake and MUST swap your card face-down with 1 of the 3 center cards without looking.",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&DRUNK, 180, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .immediate_copy()
                .writes(BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES)]
        }
    }
}

impl Drunk {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let ans = ctx
            .ask(
                irc_format!(
                    "🍺 Drunk: You MUST swap your card with center {}, {}, or {}!",
                    "1".bold(),
                    "2".bold(),
                    "3".bold(),
                ),
                |m| match Roster::parse_center_slot(&m.text) {
                    Some(c_idx) => AskMatch::accept_say(
                        c_idx,
                        format!("🍺 Locked in face-down swap with Center #{}!", c_idx + 1),
                    ),
                    None => AskMatch::reject("Drunk MUST swap! Reply 1, 2, or 3.".yellow()),
                },
            )
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<usize>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let c_idx = match ans {
            Answer::Ok(c) => c,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let auto = fastrand::usize(0..3);
                ctx.dm(
                    seat,
                    format!("🍺 Randomly swapped your card with Center #{}.", auto + 1),
                );
                auto
            }
        };
        let Some((old_self, new_self)) = board.swap_player_with_center(seat, seat, c_idx) else {
            ctx.dm(seat, "🛡️ Your card was shielded—Drunk swap prevented!".yellow());
            return;
        };
        ctx.record(irc_format!(
            "🍺 {actor_label} ({}) swapped {} with Center #{} ({}).",
            ctx.nick(seat).bold(),
            old_self.display_for_peek(),
            c_idx + 1,
            new_self.display_for_peek(),
        ));
    }
}

#[cfg(test)]
#[path = "drunk_test.rs"]
mod test;

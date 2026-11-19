use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, CenterSlot};
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
            "Wake and MUST swap your card face-down with 1 center card without looking.",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&DRUNK, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .writes(BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES)]
        }
    }
}

impl Drunk {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        // Rule (Daybreak — Sentinel): "A shield token on the Drunk's card
        // prevents the Drunk from exchanging his card with a card in the
        // center."
        if ctx.is_shielded(seat) {
            ctx.say(
                "🛡️ Drunk: Your own card has a Shield token — you cannot swap it tonight!".yellow(),
            );
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "🛡️ {label} ({}) could not swap because their own card was shielded.",
                    rctx.nick(seat).bold(),
                ));
            }));
        }

        let roster = ctx.roster;
        let hint = roster.center_choice_hint();
        let ans = ctx
            .ask(
                irc_format!(
                    "🍺 Drunk: You MUST swap your card with center {}!",
                    hint.as_str().bold(),
                ),
                move |m| match roster.parse_center_slot(&m.text) {
                    Some(slot) => AskMatch::accept_say(
                        slot,
                        format!("🍺 Locked in face-down swap with {}!", slot.label(roster)),
                    ),
                    None => AskMatch::reject(format!("Drunk MUST swap! Reply {hint}.").yellow()),
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
        ans: Answer<CenterSlot>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let slot = match ans {
            Answer::Ok(c) => c,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let auto = ctx.roster.choose_random_center_slot();
                ctx.dm(
                    seat,
                    format!("🍺 Randomly swapped your card with {}.", auto.label(ctx.roster)),
                );
                auto
            }
        };
        let Some((old_self, new_self)) = board.swap_player_with_center(seat, seat, slot) else {
            ctx.dm(seat, "🛡️ Your card was shielded—Drunk swap prevented!".yellow());
            return;
        };
        ctx.record(irc_format!(
            "🍺 {actor_label} ({}) swapped {} with {} ({}).",
            ctx.nick(seat).bold(),
            old_self.display_for_peek(),
            slot.label(ctx.roster),
            new_self.display_for_peek(),
        ));
    }
}

#[cfg(test)]
#[path = "drunk_test.rs"]
mod test;

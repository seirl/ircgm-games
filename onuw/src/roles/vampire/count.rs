use ircgm::prelude::*;

use super::vampire::VAMPIRE_PACK_STEP;
use crate::board::{BoardAccess, BoardMut};
use crate::marks::Mark;
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

pub struct Count;
pub static COUNT: Count = Count;

impl Role for Count {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "count",
            "The Count",
            GameSet::Vampire,
            Team::Vampire,
            "Vampire who MUST place the Mark of Fear on a non-Vampire player in Dusk, preventing them from waking in Night!",
        )
        .aliases(&["count", "the count"])
        .night_faction(NightFaction::Vampire)
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                VAMPIRE_PACK_STEP,
                WakeStep::solo(&COUNT, 30, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .dusk()
                    .reads(BoardAccess::MARKS)
                    .writes(BoardAccess::MARKS),
            ]
        }
    }
}

impl Count {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let card_vamps = ctx.seats_with_faction(NightFaction::Vampire);
        let valid_targets: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| i != seat && !card_vamps.contains(&i) && ctx.mark_at(i) != Mark::Vampire)
            .collect();
        if valid_targets.is_empty() {
            return Ok(SeatEffect::noop());
        }

        let non_vamps_str = ctx.format_visible_indices(&valid_targets);
        let view = ctx.view();
        let valid_for_match = valid_targets.clone();
        let prompt = irc_format!(
            "🦇 The Count: You MUST reply {} to place the Mark of Fear on a non-Vampire (suppressing their Night wake)! Non-Vampires: {non_vamps_str}",
            "<nick>".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| match view.resolve_nick(&m.text) {
                Some(idx) if valid_for_match.contains(&idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🦇 Placed {} on {}!",
                        Mark::Fear.styled(),
                        view.roster.seats[idx].user.nick().bold(),
                    ),
                ),
                Some(_) => AskMatch::reject("You cannot terrify a fellow Vampire!".yellow()),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;

        let target = ans
            .ok()
            .unwrap_or_else(|| valid_targets[fastrand::usize(..valid_targets.len())]);

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, target, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        target: usize,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        board.set_mark(target, Mark::Fear);
        ctx.record(irc_format!(
            "🦇 {actor_label} ({}) placed {} on {}.",
            ctx.nick(seat).bold(),
            Mark::Fear.styled(),
            ctx.nick(target).bold(),
        ));
    }
}

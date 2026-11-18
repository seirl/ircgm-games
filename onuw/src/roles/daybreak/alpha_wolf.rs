use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};
use crate::roles::base::werewolf::{WEREWOLF, WOLF_PACK_STEP};

pub struct AlphaWolf;
pub static ALPHA_WOLF: AlphaWolf = AlphaWolf;

impl Role for AlphaWolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "alpha_wolf",
            "Alpha Wolf",
            GameSet::Daybreak,
            Team::Werewolf,
            "Wake with the Werewolves, then MUST swap the center Wolf card with another non-Werewolf player's card!",
        )
        .aliases(&["aw", "alpha"])
        .night_faction(NightFaction::Werewolf)
        .extra_center_card("Center Wolf", "wolf", &["center_wolf", "cw"], const { &[&WEREWOLF] })
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WOLF_PACK_STEP,
                WakeStep::solo(&ALPHA_WOLF, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .writes(BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES),
            ]
        }
    }
}

impl AlphaWolf {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let wolves = ctx.known_pack_mates(NightFaction::Werewolf);
        let non_wolves = ctx.roster.all_seats().without(seat) - wolves;
        let valid_targets = non_wolves - ctx.shielded_seats();

        if valid_targets.is_empty() {
            ctx.say("🐺 Alpha Wolf: No valid non-Wolf targets exist tonight!");
            return Ok(SeatEffect::noop());
        }

        let non_wolves_str = ctx.roster.format_seats(non_wolves);
        let view = ctx.view();
        let prompt = irc_format!(
            "🐺 Alpha Wolf: You MUST swap the center Werewolf card onto a non-Wolf player ({})! Non-Wolves: {non_wolves_str}",
            "<nick>".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| match view.resolve_nick(&m.text) {
                Some(idx) if valid_targets.contains(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🐺 Swapped the center Wolf card with {}'s card!",
                        view.roster.seats[idx].user.nick().bold().red()
                    ),
                ),
                Some(idx) if view.is_shielded(idx) => {
                    AskMatch::reject("🛡️ That player's card is shielded by the Sentinel!".yellow())
                }
                Some(_) => AskMatch::reject("Pick another non-Werewolf player!".yellow()),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;

        let chosen = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let auto = valid_targets.choose_random().expect("valid_targets non-empty");
                ctx.say(format!(
                    "🐺 Randomly swapped the center Wolf card with {}'s card!",
                    ctx.nick(auto)
                ));
                auto
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, chosen, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        target: usize,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Some((old_card, planted)) =
            board.swap_player_with_extra(ALPHA_WOLF.info().id, seat, target)
        {
            ctx.record(irc_format!(
                "🐺 {actor_label} ({}) bit {}, swapping {} in for {}.",
                ctx.nick(seat).bold(),
                ctx.nick(target).bold(),
                planted.display_for_peek(),
                old_card.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "alpha_wolf_test.rs"]
mod test;

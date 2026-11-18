use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Flipper;
pub static FLIPPER: Flipper = Flipper;

impl Role for Flipper {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "flipper",
            "Flipper",
            GameSet::SuperVillains,
            Team::Village,
            "Flip another player's card face up for Day — unless it is Evil, Independent, or Innocent Bystander, in which case you flip it back face down!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&FLIPPER, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .copy_on_turn()
                .writes(BoardAccess::FACE_UP)]
        }
    }
}

impl Flipper {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🐬 Flipper: Reply {} to flip another player's card, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🐬 Passed — left all cards face down.");
                }
                match view.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => {
                        AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                    }
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ Shielded cards cannot be flipped!".yellow())
                    }
                    Some(idx) if view.is_face_up(idx) => {
                        AskMatch::reject("🔦 That player's card is already face up!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🐬 Locked in flipping {}'s card — reveals when the phase ends!",
                            view.roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Unknown player nickname (or pass).".yellow()),
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
        if let Answer::Ok(Some(target)) = ans {
            // Super Villains rulebook: Flipper flips back any card NOT on the Hero (Village) team
            // (including evil minions and Independent roles), plus teamless Innocent Bystander.
            let Some((card, stayed_face_up)) = board.reveal_player_unless(seat, target, |r| {
                r.team() != Team::Village || r.flips_back_on_reveal()
            }) else {
                return;
            };
            let actor_nick = ctx.nick(seat);
            let target_nick = ctx.nick(target);
            if !stayed_face_up {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🐬 {} is {} — you quickly flip it BACK face down!",
                        target_nick.as_str().bold(),
                        card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🐬 {actor_label} ({}) peeked {} ({}) and flipped it back face-down.",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            } else {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🐬 {} is {} — left FACE UP for everyone to see during Day!",
                        target_nick.as_str().bold(),
                        card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🐬 {actor_label} ({}) flipped {}'s card ({}) face-up!",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "flipper_test.rs"]
mod test;

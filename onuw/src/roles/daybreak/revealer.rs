use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Revealer;
pub static REVEALER: Revealer = Revealer;

impl Role for Revealer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "revealer",
            "Revealer",
            GameSet::Daybreak,
            Team::Village,
            "Flip another player's card face up for Dawn—unless it is Evil or Independent, in which case you flip it back face down!",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        format!(
            "Flip another player's card face up for Dawn—unless it is {}, in which case you flip it back face down!",
            roster.describe_non_village_roles()
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&REVEALER, 192, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .writes(BoardAccess::FACE_UP)]
        }
    }
}

impl Revealer {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.format_visible_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🔦 Revealer: Reply {} to flip their card, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🔦 Passed — left all cards face down.");
                }
                match view.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => {
                        AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                    }
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ Shielded cards cannot be flipped!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🔦 Locked in flipping {}'s card — reveals when the phase ends!",
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
            let Some((card, stayed_face_up)) = board.reveal_player_if_village(seat, target) else {
                return;
            };
            let actor_nick = ctx.nick(seat);
            let target_nick = ctx.nick(target);
            if !stayed_face_up {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🔦 {} is {} — you quickly flip it BACK face down!",
                        target_nick.as_str().bold(),
                        card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🔦 {actor_label} ({}) peeked {} ({}) and flipped it back face-down.",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            } else {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🔦 {} is {} — left FACE UP for everyone to see at Dawn!",
                        target_nick.as_str().bold(),
                        card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🔦 {actor_label} ({}) flipped {}'s card ({}) face-up!",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "revealer_test.rs"]
mod test;

use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct VillageIdiot;
pub static VILLAGE_IDIOT: VillageIdiot = VillageIdiot;

#[derive(Clone, Copy)]
enum RotateDir {
    Left,
    Right,
}

impl Role for VillageIdiot {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "village_idiot",
            "Village Idiot",
            GameSet::Daybreak,
            Team::Village,
            "May rotate ALL other unshielded players' cards one step left or right around the table (your own card stays put!).",
        )
        .aliases(&["vi", "idiot"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&VILLAGE_IDIOT, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .writes(BoardAccess::PLAYER_FACES),
            ]
        }
    }
}

impl VillageIdiot {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let prompt = irc_format!(
            "🤪 Village Idiot: Reply {} ({}), {} ({}), or {} to rotate all other unshielded cards! Players: {players_str}",
            "left".bold(),
            "-1".bold(),
            "right".bold(),
            "+1".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🤪 Passed — left all cards in place.");
                }
                match m.text.trim().to_ascii_lowercase().as_str() {
                    "left" | "l" | "-1" => AskMatch::accept_say(
                        Some(RotateDir::Left),
                        "🤪 Locked in rotating other cards LEFT!",
                    ),
                    "right" | "r" | "+1" => AskMatch::accept_say(
                        Some(RotateDir::Right),
                        "🤪 Locked in rotating other cards RIGHT!",
                    ),
                    _ => AskMatch::reject("Reply left, right, or pass.".yellow()),
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
        ans: Answer<Option<RotateDir>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        let Answer::Ok(Some(dir)) = ans else {
            ctx.record(irc_format!(
                "🤪 {actor_label} ({}) did not rotate cards.",
                actor_nick.bold(),
            ));
            return;
        };

        let rotate_right = matches!(dir, RotateDir::Right);
        let moved = board.rotate_other_unshielded(seat, rotate_right);
        if moved >= 2 {
            let dir_str = match dir {
                RotateDir::Left => "left",
                RotateDir::Right => "right",
            };
            ctx.record(irc_format!(
                "🤪 {actor_label} ({}) rotated {moved} unshielded cards {dir_str}.",
                actor_nick.bold(),
            ));
        } else {
            ctx.dm(
                seat,
                "🛡️ Village Idiot: Fewer than 2 unshielded other cards could be rotated!"
                    .yellow(),
            );
            ctx.record(irc_format!(
                "🤪 {actor_label} ({}) could not rotate (< 2 unshielded other cards).",
                actor_nick.bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "village_idiot_test.rs"]
mod test;

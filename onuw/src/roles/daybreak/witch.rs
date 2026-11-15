use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Witch;
pub static WITCH: Witch = Witch;

impl Role for Witch {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "witch",
            "Witch",
            GameSet::Daybreak,
            Team::Village,
            "May look at 1 center card; if you do, you MUST swap it with any player's card (including your own)!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(165, Self::prompt).reads_board().mutates())
    }
}

impl Witch {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let step1 = ctx
                .ask(
                    "🧙‍♀️ Witch: Reply `1`, `2`, or `3` to view a center card (you MUST then give it to someone!), or `pass`:",
                    |m| {
                        if Roster::is_pass(&m.text) {
                            return AskMatch::accept(None);
                        }
                        match Roster::parse_center_slot(&m.text) {
                            Some(c_idx) => AskMatch::accept(Some(c_idx)),
                            None => AskMatch::reject("Reply `1`, `2`, `3`, or `pass`.".yellow()),
                        }
                    },
                )
                .await?;

            let Answer::Ok(Some(c_idx)) = step1 else {
                return Ok(SeatEffect::new(move |label, _board, rctx| {
                    rctx.record(irc_format!(
                        "🧙‍♀️ {label} ({}) did not brew anything.",
                        rctx.nick(seat).bold(),
                    ));
                }));
            };

            let seen_center = ctx.board().center[c_idx];
            ctx.say(irc_format!(
                "🧙‍♀️ Center #{} is {}!",
                c_idx + 1,
                seen_center.display_for_peek()
            ));

            let unshielded: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| !ctx.shielded.contains(&i))
                .collect();
            if unshielded.is_empty() {
                return Ok(SeatEffect::noop());
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let step2 = ctx
                .ask(
                    format!(
                        "🧙‍♀️ Give Center #{} to which player (`<nick>`, including yourself)? Players: {}",
                        c_idx + 1,
                        roster.format_players_marking_shields(shielded)
                    ),
                    move |m| match roster.resolve_nick(&m.text) {
                        Some(idx) if shielded.contains(&idx) => {
                            AskMatch::reject("🛡️ That player is shielded!".yellow())
                        }
                        Some(idx) => AskMatch::accept_say(
                            idx,
                            irc_format!(
                                "🧙‍♀️ Swapped Center #{} onto {}!",
                                c_idx + 1,
                                roster.seats[idx].user.nick().bold(),
                            ),
                        ),
                        None => AskMatch::reject("Unknown player nickname.".yellow()),
                    },
                )
                .await?;

            let target = match step2 {
                Answer::Ok(idx) => idx,
                Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                    let auto = if !ctx.shielded.contains(&seat) {
                        seat
                    } else {
                        unshielded[fastrand::usize(..unshielded.len())]
                    };
                    ctx.say(format!(
                        "🧙‍♀️ Time expired—swapped Center #{} with {}!",
                        c_idx + 1,
                        ctx.nick(auto)
                    ));
                    auto
                }
            };

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, c_idx, target, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        c_idx: usize,
        target: usize,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if !board.is_shielded(target) {
            board.record_card_interaction(seat);
            let center_card = board.center[c_idx];
            let old_player_card = board.players[target];
            std::mem::swap(&mut board.center[c_idx], &mut board.players[target]);
            ctx.record(irc_format!(
                "🧙‍♀️ {actor_label} ({}) peeked Center #{} ({}) and swapped it onto {} (moving {} into Center #{}).",
                ctx.nick(seat).bold(),
                c_idx + 1,
                center_card.display_for_peek(),
                ctx.nick(target).bold(),
                old_player_card.display_for_peek(),
                c_idx + 1,
            ));
        }
    }
}

#[cfg(test)]
#[path = "witch_test.rs"]
mod test;

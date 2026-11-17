use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct VoodooLou;
pub static VOODOO_LOU: VoodooLou = VoodooLou;

impl Role for VoodooLou {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "voodoo_lou",
            "Voodoo Lou",
            GameSet::SuperVillains,
            Team::Village,
            "May look at 1 center card; if you do, you MUST swap it with any player's card (including your own)!",
        )
        .aliases(&["voodoolou", "voodoo"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&VOODOO_LOU, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .reads(BoardAccess::CENTER_FACES)
                    .writes(BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES),
            ]
        }
    }
}

impl VoodooLou {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let step1 = ctx
            .ask(
                irc_format!(
                    "🪡 Voodoo Lou: Reply {}, {}, or {} to view a center card (you MUST then swap it onto someone!), or {}:",
                    "1".bold(),
                    "2".bold(),
                    "3".bold(),
                    "pass".bold(),
                ),
                |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept_say(None, "🪡 Passed — left center untouched.");
                    }
                    match Roster::parse_center_slot(&m.text) {
                        Some(c_idx) => AskMatch::accept(Some(c_idx)),
                        None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
                    }
                },
            )
            .await?;

        let Answer::Ok(Some(c_idx)) = step1 else {
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "🪡 {label} ({}) did not use voodoo tonight.",
                    rctx.nick(seat).bold(),
                ));
            }));
        };

        let seen_center = ctx.peek_center_card(c_idx);
        ctx.say(irc_format!(
            "🪡 Center #{} is {}!",
            c_idx + 1,
            seen_center.display_for_peek(),
        ));

        let unshielded = ctx.roster.all_seats() - ctx.shielded_seats();
        if unshielded.is_empty() {
            ctx.say(
                "🛡️ All player cards are shielded — you cannot swap the center card onto anyone!"
                    .yellow(),
            );
            return Ok(SeatEffect::new(move |label, board, rctx| {
                board.record_prompt_peek(seat);
                rctx.record(irc_format!(
                    "🪡 {label} ({}) peeked Center #{} ({}), but all player cards were shielded.",
                    rctx.nick(seat).bold(),
                    c_idx + 1,
                    seen_center.display_for_peek(),
                ));
            }));
        }

        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let step2 = ctx
            .ask(
                irc_format!(
                    "🪡 Give Center #{} to which player ({}, including yourself)? Players: {players_str}",
                    c_idx + 1,
                    "<nick>".bold(),
                ),
                move |m| match view.resolve_nick(&m.text) {
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ That player is shielded!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🪡 Swapped Center #{} onto {}!",
                            c_idx + 1,
                            view.roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Unknown player nickname.".yellow()),
                },
            )
            .await?;

        let target = match step2 {
            Answer::Ok(idx) => idx,
            _ => {
                let auto = if unshielded.contains(seat) {
                    seat
                } else {
                    unshielded.choose_random().expect("unshielded non-empty")
                };
                ctx.say(format!(
                    "🪡 Time expired — swapped Center #{} with {}!",
                    c_idx + 1,
                    ctx.nick(auto)
                ));
                auto
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, c_idx, target, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        c_idx: usize,
        target: usize,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Some((old_player_card, center_card)) =
            board.swap_player_with_center(seat, target, c_idx)
        {
            ctx.record(irc_format!(
                "🪡 {actor_label} ({}) peeked Center #{} ({}) and swapped it onto {} (moving {} into Center #{}).",
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
#[path = "voodoo_lou_test.rs"]
mod test;

use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, CenterSlot, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&WITCH, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .reads(BoardAccess::CENTER_FACES)
                .writes(BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES)]
        }
    }
}

impl Witch {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let choice_hint = roster.center_choice_hint();
        let or_pass_hint = roster.center_or_pass_hint();
        let step1 = ctx
            .ask(
                irc_format!(
                    "🧙‍♀️ Witch: Reply {} to view a center card (you MUST then give it to someone!), or {}:",
                    choice_hint.bold(),
                    "pass".bold(),
                ),
                move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept_say(None, "🧙‍♀️ Passed — brewed nothing tonight.");
                    }
                    match roster.parse_center_slot(&m.text) {
                        Some(slot) => AskMatch::accept(Some(slot)),
                        None => AskMatch::reject(format!("Reply {or_pass_hint}.").yellow()),
                    }
                },
            )
            .await?;

        let Answer::Ok(Some(slot)) = step1 else {
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "🧙‍♀️ {label} ({}) did not brew anything.",
                    rctx.nick(seat).bold(),
                ));
            }));
        };

        let slot_label = slot.label(roster);
        let seen_center = ctx.peek_center_card(slot);
        ctx.say(irc_format!(
            "🧙‍♀️ {slot_label} is {}!",
            seen_center.display_for_peek()
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
                    "🧙‍♀️ {label} ({}) peeked {} ({}), but all player cards were shielded.",
                    rctx.nick(seat).bold(),
                    slot.label(rctx.roster),
                    seen_center.display_for_peek(),
                ));
            }));
        }

        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let step2 = ctx
            .ask(
                irc_format!(
                    "🧙‍♀️ Give {slot_label} to which player ({}, including yourself)? Players: {players_str}",
                    "<nick>".bold(),
                ),
                move |m| match view.resolve_nick(&m.text) {
                    Some(idx) if view.is_shielded(idx) => {
                        AskMatch::reject("🛡️ That player is shielded!".yellow())
                    }
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🧙‍♀️ Swapped {} onto {}!",
                            slot.label(view.roster),
                            view.roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Unknown player nickname.".yellow()),
                },
            )
            .await?;

        let target = match step2 {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let auto = if unshielded.contains(seat) {
                    seat
                } else {
                    unshielded.choose_random().expect("unshielded non-empty")
                };
                ctx.say(format!(
                    "🧙‍♀️ Time expired—swapped {slot_label} with {}!",
                    ctx.nick(auto)
                ));
                auto
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, slot, target, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        slot: CenterSlot,
        target: usize,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Some((old_player_card, center_card)) =
            board.swap_player_with_center(seat, target, slot)
        {
            let slot_label = slot.label(ctx.roster);
            ctx.record(irc_format!(
                "🧙‍♀️ {actor_label} ({}) peeked {slot_label} ({}) and swapped it onto {} (moving {} into {slot_label}).",
                ctx.nick(seat).bold(),
                center_card.display_for_peek(),
                ctx.nick(target).bold(),
                old_player_card.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "witch_test.rs"]
mod test;

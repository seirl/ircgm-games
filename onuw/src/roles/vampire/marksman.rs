use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Marksman;
pub static MARKSMAN: Marksman = Marksman;

#[derive(Clone, Copy, Debug)]
enum MarksmanChoice {
    Pass,
    CardOnly(usize),
    CardAndMark(usize, usize),
}

impl Role for Marksman {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "marksman",
            "Marksman",
            GameSet::Vampire,
            Team::Village,
            "Wake in Night to view ONE player's card AND a DIFFERENT player's Mark!",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has_marks() {
            self.info().summary.to_owned()
        } else {
            "Wake in Night to view 1 other player's card!".to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(158, Self::prompt))
    }
}

impl Marksman {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let has_marks = roster.deck_has_marks();
            let players_str = roster.format_players_marking_shields(shielded);
            let prompt = if has_marks {
                format!(
                    "🎯 Marksman: Reply `<card_nick> <mark_nick>` (two DIFFERENT players) or `pass`. Players: {players_str}"
                )
            } else {
                format!(
                    "🎯 Marksman: Reply `<nick>` to view another player's card, or `pass`. Players: {players_str}"
                )
            };

            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(MarksmanChoice::Pass);
                    }
                    let parts: Vec<&str> = m.text.split_whitespace().collect();
                    let (card_tok, mark_tok) = match parts.as_slice() {
                        [card_tok] if !has_marks => (*card_tok, None),
                        [card_tok, mark_tok] if has_marks => (*card_tok, Some(*mark_tok)),
                        _ if has_marks => {
                            return AskMatch::reject(
                                "Usage: `<card_nick> <mark_nick>` or `pass`.".yellow(),
                            );
                        }
                        _ => {
                            return AskMatch::reject(
                                "There are no Marks in this game — reply `<nick>` to view another player's card, or `pass`."
                                    .yellow(),
                            );
                        }
                    };
                    let Some(card_idx) = roster.resolve_nick(card_tok) else {
                        return AskMatch::reject("Unknown player nickname.".yellow());
                    };
                    if card_idx == seat {
                        return AskMatch::reject("You cannot inspect your own card!".yellow());
                    }
                    if shielded.contains(&card_idx) {
                        return AskMatch::reject(
                            "🛡️ That card is shielded by the Sentinel!".yellow(),
                        );
                    }
                    if let Some(mark_tok) = mark_tok {
                        let Some(mark_idx) = roster.resolve_nick(mark_tok) else {
                            return AskMatch::reject("Unknown player nickname.".yellow());
                        };
                        if card_idx == mark_idx {
                            return AskMatch::reject(
                                "Card target and Mark target must be DIFFERENT players!".yellow(),
                            );
                        }
                        AskMatch::accept(MarksmanChoice::CardAndMark(card_idx, mark_idx))
                    } else {
                        AskMatch::accept(MarksmanChoice::CardOnly(card_idx))
                    }
                })
                .await?;

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, ans, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<MarksmanChoice>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match ans.ok().unwrap_or(MarksmanChoice::Pass) {
            MarksmanChoice::Pass => {}
            MarksmanChoice::CardOnly(card_idx) => {
                if board.is_shielded(card_idx) {
                    return;
                }
                board.record_card_interaction(seat);
                let seen_card = board.players[card_idx];
                let target_nick = ctx.nick(card_idx);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🎯 Marksman: {}'s card is {}!",
                        target_nick.as_str().bold(),
                        seen_card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🎯 {actor_label} ({}) viewed {target_nick}'s card ({}).",
                    actor_nick.bold(),
                    seen_card.display_for_peek(),
                ));
            }
            MarksmanChoice::CardAndMark(card_idx, mark_idx) => {
                if board.is_shielded(card_idx) {
                    return;
                }
                board.record_card_interaction(seat);
                let seen_card = board.players[card_idx];
                let seen_mark = board.marks[mark_idx];
                let card_nick = ctx.nick(card_idx);
                let mark_nick = ctx.nick(mark_idx);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🎯 Marksman: {}'s card is {}, and {}'s mark is {}!",
                        card_nick.as_str().bold(),
                        seen_card.display_for_peek(),
                        mark_nick.as_str().bold(),
                        seen_mark.styled(),
                    ),
                );
                ctx.record(irc_format!(
                    "🎯 {actor_label} ({}) viewed {}'s card ({}) and {}'s mark ({}).",
                    actor_nick.bold(),
                    card_nick.bold(),
                    seen_card.display_for_peek(),
                    mark_nick.bold(),
                    seen_mark.styled(),
                ));
            }
        }
    }
}

use ircgm::prelude::*;

use crate::board::{BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Marksman;
pub static MARKSMAN: Marksman = Marksman;

#[derive(Clone, Copy, Debug)]
enum MarksmanChoice {
    Pass,
    CardOnly(usize),
    MarkOnly(usize),
    CardAndMark(usize, usize),
}

impl Role for Marksman {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "marksman",
            "Marksman",
            GameSet::Vampire,
            Team::Village,
            "Wake in Night to view one other player's card and/or a different other player's Mark!",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has_marks() {
            self.info().summary.to_owned()
        } else {
            "Wake in Night to view 1 other player's card!".to_owned()
        }
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&MARKSMAN, 158, |s, ctx| Box::pin(Self::prompt(s, ctx))).immediate_copy()]
        }
    }
}

impl Marksman {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let view = ctx.view();
        let has_marks = roster.deck_has_marks();
        let players_str = ctx.format_visible_players();
        let prompt = if has_marks {
            irc_format!(
                "🎯 Marksman: Reply {} (two DIFFERENT other players), {}, {}, or {}. Players: {players_str}",
                "<card_nick> <mark_nick>".bold(),
                "card <nick>".bold(),
                "mark <nick>".bold(),
                "pass".bold(),
            )
        } else {
            irc_format!(
                "🎯 Marksman: Reply {} to view another player's card, or {}. Players: {players_str}",
                "<nick>".bold(),
                "pass".bold(),
            )
        };

        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(
                        MarksmanChoice::Pass,
                        "🎯 Passed — not inspecting anyone.",
                    );
                }
                let parts: Vec<&str> = m.text.split_whitespace().collect();
                let (card_tok, mark_tok) = match parts.as_slice() {
                    [w, nick] if w.eq_ignore_ascii_case("card") => (Some(*nick), None),
                    [w, nick] if has_marks && w.eq_ignore_ascii_case("mark") => {
                        (None, Some(*nick))
                    }
                    [card_tok] if !has_marks => (Some(*card_tok), None),
                    [card_tok, mark_tok] if has_marks => (Some(*card_tok), Some(*mark_tok)),
                    _ if has_marks => {
                        return AskMatch::reject(
                            "Usage: <card_nick> <mark_nick>, card <nick>, mark <nick>, or pass."
                                .yellow(),
                        );
                    }
                    _ => {
                        return AskMatch::reject(
                            "There are no Marks in this game — reply <nick> to view another player's card, or pass."
                                .yellow(),
                        );
                    }
                };
                let card_idx = if let Some(ct) = card_tok {
                    let Some(idx) = roster.resolve_nick(ct) else {
                        return AskMatch::reject("Unknown player nickname.".yellow());
                    };
                    if idx == seat {
                        return AskMatch::reject("You cannot inspect your own card!".yellow());
                    }
                    if view.is_shielded(idx) {
                        return AskMatch::reject(
                            "🛡️ That card is shielded by the Sentinel!".yellow(),
                        );
                    }
                    Some(idx)
                } else {
                    None
                };
                let mark_idx = if let Some(mt) = mark_tok {
                    let Some(idx) = roster.resolve_nick(mt) else {
                        return AskMatch::reject("Unknown player nickname.".yellow());
                    };
                    if idx == seat {
                        return AskMatch::reject("You cannot inspect your own Mark!".yellow());
                    }
                    Some(idx)
                } else {
                    None
                };
                match (card_idx, mark_idx) {
                    (Some(c), Some(mk)) if c == mk => AskMatch::reject(
                        "Card target and Mark target must be DIFFERENT players!".yellow(),
                    ),
                    (Some(c), Some(mk)) => AskMatch::accept_say(
                        MarksmanChoice::CardAndMark(c, mk),
                        irc_format!(
                            "🎯 Locked in viewing {}'s card and {}'s mark — reveals when the phase ends!",
                            roster.seats[c].user.nick().bold(),
                            roster.seats[mk].user.nick().bold(),
                        ),
                    ),
                    (Some(c), None) => AskMatch::accept_say(
                        MarksmanChoice::CardOnly(c),
                        irc_format!(
                            "🎯 Locked in viewing {}'s card — reveals when the phase ends!",
                            roster.seats[c].user.nick().bold(),
                        ),
                    ),
                    (None, Some(mk)) => AskMatch::accept_say(
                        MarksmanChoice::MarkOnly(mk),
                        irc_format!(
                            "🎯 Locked in viewing {}'s mark — reveals when the phase ends!",
                            roster.seats[mk].user.nick().bold(),
                        ),
                    ),
                    (None, None) => AskMatch::accept_say(
                        MarksmanChoice::Pass,
                        "🎯 Passed — not inspecting anyone.",
                    ),
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
        ans: Answer<MarksmanChoice>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match ans.ok().unwrap_or(MarksmanChoice::Pass) {
            MarksmanChoice::Pass => {}
            MarksmanChoice::CardOnly(card_idx) => {
                let Some(seen_card) = board.peek_player(seat, card_idx) else {
                    return;
                };
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
            MarksmanChoice::MarkOnly(mark_idx) => {
                let seen_mark = board.mark_at(mark_idx);
                let mark_nick = ctx.nick(mark_idx);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🎯 Marksman: {}'s mark is {}!",
                        mark_nick.as_str().bold(),
                        seen_mark.styled(),
                    ),
                );
                ctx.record(irc_format!(
                    "🎯 {actor_label} ({}) viewed {}'s mark ({}).",
                    actor_nick.bold(),
                    mark_nick.bold(),
                    seen_mark.styled(),
                ));
            }
            MarksmanChoice::CardAndMark(card_idx, mark_idx) => {
                let Some(seen_card) = board.peek_player(seat, card_idx) else {
                    return;
                };
                let seen_mark = board.mark_at(mark_idx);
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

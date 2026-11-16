use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake};

pub struct Gremlin;
pub static GREMLIN: Gremlin = Gremlin;

#[derive(Clone, Copy, Debug)]
enum GremlinChoice {
    Pass,
    SwapCards(usize, usize),
    SwapMarks(usize, usize),
}

impl Role for Gremlin {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "gremlin",
            "Gremlin",
            GameSet::Vampire,
            Team::Village,
            "Wake in Night to swap EITHER two players' cards (cards <nick1> <nick2>) OR two players' Marks (marks <nick1> <nick2>), including yourself, without looking!",
        )
        .aliases(&["kobold"])
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has_marks() {
            self.info().summary.to_owned()
        } else {
            "Wake in Night to swap ANY two players' cards (including yourself), without looking!"
                .to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(
            Wake::solo(176, |seat, ctx| Box::pin(Self::prompt(seat, ctx)))
                .mutates()
                .follow_on_copy(),
        )
    }
}

impl Gremlin {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let (roster, shielded) = (ctx.roster, ctx.shielded);
        let has_marks = roster.deck_has_marks();
        let players_str = roster.format_players_marking_shields(shielded);
        let prompt = if has_marks {
            irc_format!(
                "👺 Gremlin: Reply {} OR {} (any two players, including yourself), or {}. Players: {players_str}",
                "cards <nick1> <nick2>".bold(),
                "marks <nick1> <nick2>".bold(),
                "pass".bold(),
            )
        } else {
            irc_format!(
                "👺 Gremlin: Reply {} (any two players, including yourself) to swap their cards, or {}. Players: {players_str}",
                "<nick1> <nick2>".bold(),
                "pass".bold(),
            )
        };

        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    let msg = if has_marks {
                        "👺 Passed — left all cards and Marks untouched."
                    } else {
                        "👺 Passed — left all cards untouched."
                    };
                    return AskMatch::accept_say(GremlinChoice::Pass, msg.cyan());
                }

                let parts: Vec<&str> = m.text.split_whitespace().collect();
                let (swap_cards, tok_a, tok_b) = match parts.as_slice() {
                    [a, b] if !has_marks => (true, *a, *b),
                    [kind, a, b] => match kind.to_ascii_lowercase().as_str() {
                        "cards" | "card" | "c" => (true, *a, *b),
                        "marks" | "mark" | "m" if has_marks => (false, *a, *b),
                        "marks" | "mark" | "m" => {
                            return AskMatch::reject(
                                "There are no Marks in this game — reply <nick1> <nick2> to swap cards, or pass."
                                    .yellow(),
                            );
                        }
                        _ if has_marks => {
                            return AskMatch::reject(
                                "Start with cards or marks (e.g. cards Alice Bob or marks Bob Carol), or pass."
                                    .yellow(),
                            );
                        }
                        _ => {
                            return AskMatch::reject(
                                "Usage: <nick1> <nick2> (two players, including yourself) or pass."
                                    .yellow(),
                            );
                        }
                    },
                    _ if has_marks => {
                        return AskMatch::reject(
                            "Usage: cards <nick1> <nick2>, marks <nick1> <nick2>, or pass."
                                .yellow(),
                        );
                    }
                    _ => {
                        return AskMatch::reject(
                            "Usage: <nick1> <nick2> (two players, including yourself) or pass."
                                .yellow(),
                        );
                    }
                };

                let (Some(a), Some(b)) =
                    (roster.resolve_nick(tok_a), roster.resolve_nick(tok_b))
                else {
                    return AskMatch::reject("Unknown player nickname.".yellow());
                };
                if a == b {
                    return AskMatch::reject("Pick TWO distinct players!".yellow());
                }
                if swap_cards && (shielded.contains(&a) || shielded.contains(&b)) {
                    let msg = if has_marks {
                        "🛡️ Shielded cards cannot be swapped (though Marks can)!"
                    } else {
                        "🛡️ Shielded cards cannot be swapped!"
                    };
                    return AskMatch::reject(msg.yellow());
                }
                if swap_cards {
                    AskMatch::accept_say(
                        GremlinChoice::SwapCards(a, b),
                        irc_format!(
                            "👺 Swapped {}'s card and {}'s card!",
                            roster.seats[a].user.nick().bold(),
                            roster.seats[b].user.nick().bold(),
                        ),
                    )
                } else {
                    AskMatch::accept_say(
                        GremlinChoice::SwapMarks(a, b),
                        irc_format!(
                            "👺 Swapped {}'s Mark and {}'s Mark!",
                            roster.seats[a].user.nick().bold(),
                            roster.seats[b].user.nick().bold(),
                        ),
                    )
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
        ans: Answer<GremlinChoice>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match ans.ok().unwrap_or(GremlinChoice::Pass) {
            GremlinChoice::Pass => {
                if ctx.roster.deck_has_marks() {
                    ctx.record(irc_format!(
                        "👺 {actor_label} ({}) left all cards and Marks untouched.",
                        actor_nick.bold(),
                    ));
                } else {
                    ctx.record(irc_format!(
                        "👺 {actor_label} ({}) did not swap any cards.",
                        actor_nick.bold(),
                    ));
                }
            }
            GremlinChoice::SwapCards(a, b) => {
                if board.is_shielded(a) || board.is_shielded(b) {
                    return;
                }
                board.record_card_interaction(seat);
                let card_a = board.players[a].display_for_peek();
                let card_b = board.players[b].display_for_peek();
                board.players.swap(a, b);
                ctx.record(irc_format!(
                    "👺 {actor_label} ({}) swapped {}'s card ({card_a}) and {}'s card ({card_b}).",
                    actor_nick.bold(),
                    ctx.nick(a).bold(),
                    ctx.nick(b).bold(),
                ));
            }
            GremlinChoice::SwapMarks(a, b) => {
                let mark_a = board.marks[a].styled();
                let mark_b = board.marks[b].styled();
                board.marks.swap(a, b);
                ctx.record(irc_format!(
                    "👺 {actor_label} ({}) swapped {}'s Mark ({mark_a}) and {}'s Mark ({mark_b}).",
                    actor_nick.bold(),
                    ctx.nick(a).bold(),
                    ctx.nick(b).bold(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "gremlin_test.rs"]
mod test;

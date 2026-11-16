use std::collections::HashSet;

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake};

pub struct Seer;
pub static SEER: Seer = Seer;

#[derive(Clone, Copy)]
enum SeerChoice {
    Player(usize),
    Center(usize, usize),
    Pass,
}

impl Role for Seer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "seer",
            "Seer",
            GameSet::Base,
            Team::Village,
            "Wake to view EITHER 1 other player's card OR 2 center cards.",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(150, |seat, ctx| {
            Box::pin(Self::prompt(seat, ctx))
        }))
    }
}

impl Seer {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let (roster, shielded) = (ctx.roster, ctx.shielded);
        let prompt = irc_format!(
            "🔮 Seer: Reply {} (or {}) OR {}, or {}. Players: {}",
            "player <nick>".bold(),
            "<nick>".bold(),
            "center <1-3> <1-3>".bold(),
            "pass".bold(),
            roster.format_players_marking_shields(shielded),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                parse_seer_choice(roster, shielded, seat, &m.text)
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<SeerChoice>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match ans {
            Answer::Ok(SeerChoice::Player(idx)) if !board.is_shielded(idx) => {
                board.record_card_interaction(seat);
                let card = board.players[idx];
                let target_nick = ctx.nick(idx);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🔮 {}'s card is: {}",
                        target_nick.as_str().bold(),
                        card.display_for_peek()
                    ),
                );
                ctx.record(irc_format!(
                    "🔮 {actor_label} ({}) viewed {}'s card ({}).",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            }
            Answer::Ok(SeerChoice::Center(a, b)) => {
                board.record_card_interaction(seat);
                let (ca, cb) = (board.center[a], board.center[b]);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🔮 Center #{} is {}, Center #{} is {}.",
                        a + 1,
                        ca.display_for_peek(),
                        b + 1,
                        cb.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🔮 {actor_label} ({}) viewed Center #{} ({}) & Center #{} ({}).",
                    actor_nick.bold(),
                    a + 1,
                    ca.display_for_peek(),
                    b + 1,
                    cb.display_for_peek(),
                ));
            }
            _ => ctx.record(irc_format!(
                "🔮 {actor_label} ({}) did not view any cards.",
                actor_nick.bold(),
            )),
        }
    }
}

fn parse_seer_choice(
    roster: &Roster,
    shielded: &HashSet<usize>,
    actor_seat: usize,
    raw: &str,
) -> AskMatch<SeerChoice> {
    if Roster::is_pass(raw) {
        return AskMatch::accept_say(SeerChoice::Pass, "🔮 Passed — not viewing any cards.");
    }
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts
        .first()
        .is_some_and(|w| w.eq_ignore_ascii_case("center") || w.eq_ignore_ascii_case("c"))
    {
        if parts.len() != 3 {
            return AskMatch::reject("Usage: center <1-3> <1-3>.".yellow());
        }
        let (Some(a), Some(b)) = (
            Roster::parse_center_slot(parts[1]),
            Roster::parse_center_slot(parts[2]),
        ) else {
            return AskMatch::reject("Center slots must be 1, 2, or 3.".yellow());
        };
        if a == b {
            return AskMatch::reject("Pick two DIFFERENT center slots (1..=3).".yellow());
        }
        return AskMatch::accept_say(
            SeerChoice::Center(a, b),
            format!(
                "🔮 Locked in Center #{} & Center #{} — cards reveal when the phase ends!",
                a + 1,
                b + 1
            ),
        );
    }

    if parts.len() == 2
        && parts[0].to_ascii_lowercase().starts_with('c')
        && parts[1].to_ascii_lowercase().starts_with('c')
    {
        if let (Some(a), Some(b)) = (
            Roster::parse_center_slot(parts[0]),
            Roster::parse_center_slot(parts[1]),
        ) {
            if a != b {
                return AskMatch::accept_say(
                    SeerChoice::Center(a, b),
                    format!(
                        "🔮 Locked in Center #{} & Center #{} — cards reveal when the phase ends!",
                        a + 1,
                        b + 1
                    ),
                );
            }
        }
    }

    let token = if parts
        .first()
        .is_some_and(|w| w.eq_ignore_ascii_case("player") || w.eq_ignore_ascii_case("p"))
    {
        if parts.len() != 2 {
            return AskMatch::reject("Usage: player <nick>.".yellow());
        }
        parts[1]
    } else if parts.len() == 1 {
        parts[0]
    } else {
        return AskMatch::reject(
            "Reply player <nick> (or <nick>), center <1-3> <1-3>, or pass.".yellow(),
        );
    };

    match roster.resolve_nick(token) {
        Some(idx) if idx == actor_seat => {
            AskMatch::reject("You cannot look at your own card!".yellow())
        }
        Some(idx) if shielded.contains(&idx) => AskMatch::reject(
            "🛡️ That player's card has a Shield token and cannot be viewed!".yellow(),
        ),
        Some(idx) => AskMatch::accept_say(
            SeerChoice::Player(idx),
            irc_format!(
                "🔮 Locked in viewing {}'s card — reveals when the phase ends!",
                roster.seats[idx].user.nick().bold(),
            ),
        ),
        None => AskMatch::reject("Unknown player nickname.".yellow()),
    }
}

#[cfg(test)]
#[path = "seer_test.rs"]
mod test;

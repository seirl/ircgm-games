use ircgm::prelude::*;

use crate::board::{BoardMut, Roster};
use crate::role::{
    GameSet, PromptCtx, PromptView, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

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

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::solo(&SEER, 150, |s, ctx| Box::pin(Self::prompt(s, ctx)))] }
    }
}

impl Seer {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🔮 Seer: Reply {} (or {}) OR {}, or {}. Players: {players_str}",
            "player <nick>".bold(),
            "<nick>".bold(),
            "center <1-3> <1-3>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| parse_seer_choice(view, seat, &m.text))
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<SeerChoice>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match ans {
            Answer::Ok(SeerChoice::Player(idx)) => {
                if let Some(card) = board.peek_player(seat, idx) {
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
            }
            Answer::Ok(SeerChoice::Center(a, b)) => {
                let ca = board.peek_center(seat, a);
                let cb = board.peek_center(seat, b);
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

fn parse_seer_choice(view: PromptView<'_>, actor_seat: usize, raw: &str) -> AskMatch<SeerChoice> {
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

    match view.resolve_nick(token) {
        Some(idx) if idx == actor_seat => {
            AskMatch::reject("You cannot look at your own card!".yellow())
        }
        Some(idx) if view.is_shielded(idx) => AskMatch::reject(
            "🛡️ That player's card has a Shield token and cannot be viewed!".yellow(),
        ),
        Some(idx) => AskMatch::accept_say(
            SeerChoice::Player(idx),
            irc_format!(
                "🔮 Locked in viewing {}'s card — reveals when the phase ends!",
                view.roster.seats[idx].user.nick().bold(),
            ),
        ),
        None => AskMatch::reject("Unknown player nickname.".yellow()),
    }
}

#[cfg(test)]
#[path = "seer_test.rs"]
mod test;

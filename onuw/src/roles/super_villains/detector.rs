use ircgm::prelude::*;

use crate::board::{BoardMut, CenterSlot, Roster};
use crate::role::{
    GameSet, PromptCtx, PromptView, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

pub struct Detector;
pub static DETECTOR: Detector = Detector;

#[derive(Clone, Copy)]
enum DetectorChoice {
    Player(usize),
    Center(CenterSlot, CenterSlot),
    Pass,
}

impl Role for Detector {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "detector",
            "Detector",
            GameSet::SuperVillains,
            Team::Village,
            "Wake to view EITHER 1 other player's card OR 2 center cards.",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::solo(&DETECTOR, |s, ctx| Box::pin(Self::prompt(s, ctx)))] }
    }
}

impl Detector {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let center_usage = ctx.roster.center_pair_usage();
        let view = ctx.view();
        let prompt = irc_format!(
            "📡 Detector: Reply {} (or {}) OR {}, or {}. Players: {players_str}",
            "player <nick>".bold(),
            "<nick>".bold(),
            center_usage.bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| parse_detector_choice(view, seat, &m.text))
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<DetectorChoice>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match ans {
            Answer::Ok(DetectorChoice::Player(idx)) => {
                if let Some(card) = board.peek_player(seat, idx) {
                    let target_nick = ctx.nick(idx);
                    ctx.dm(
                        seat,
                        irc_format!(
                            "📡 {}'s card is: {}",
                            target_nick.as_str().bold(),
                            card.display_for_peek(),
                        ),
                    );
                    ctx.record(irc_format!(
                        "📡 {actor_label} ({}) detected {}'s card ({}).",
                        actor_nick.bold(),
                        target_nick.bold(),
                        card.display_for_peek(),
                    ));
                }
            }
            Answer::Ok(DetectorChoice::Center(a, b)) => {
                let ca = board.peek_center(seat, a);
                let cb = board.peek_center(seat, b);
                let la = a.label(ctx.roster);
                let lb = b.label(ctx.roster);
                ctx.dm(
                    seat,
                    irc_format!(
                        "📡 {la} is {}, {lb} is {}.",
                        ca.display_for_peek(),
                        cb.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "📡 {actor_label} ({}) detected {la} ({}) & {lb} ({}).",
                    actor_nick.bold(),
                    ca.display_for_peek(),
                    cb.display_for_peek(),
                ));
            }
            _ => ctx.record(irc_format!(
                "📡 {actor_label} ({}) did not detect any cards.",
                actor_nick.bold(),
            )),
        }
    }
}

fn parse_detector_choice(
    view: PromptView<'_>,
    actor_seat: usize,
    raw: &str,
) -> AskMatch<DetectorChoice> {
    if Roster::is_pass(raw) {
        return AskMatch::accept_say(DetectorChoice::Pass, "📡 Passed — not scanning any cards.");
    }
    let roster = view.roster;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts
        .first()
        .is_some_and(|w| w.eq_ignore_ascii_case("center") || w.eq_ignore_ascii_case("c"))
    {
        if parts.len() != 3 {
            return AskMatch::reject(format!("Usage: {}.", roster.center_pair_usage()).yellow());
        }
        let (Some(a), Some(b)) = (
            roster.parse_center_slot(parts[1]),
            roster.parse_center_slot(parts[2]),
        ) else {
            return AskMatch::reject(
                format!("Center slots must be {}.", roster.center_choice_hint()).yellow(),
            );
        };
        if a == b {
            return AskMatch::reject("Pick two DIFFERENT center slots.".yellow());
        }
        return AskMatch::accept_say(
            DetectorChoice::Center(a, b),
            format!(
                "📡 Locked in {} & {} — cards reveal when the phase ends!",
                a.label(roster),
                b.label(roster)
            ),
        );
    }

    if parts.len() == 2 {
        if let (Some(a), Some(b)) = (
            roster.parse_center_slot(parts[0]),
            roster.parse_center_slot(parts[1]),
        ) {
            if a != b
                && view.resolve_nick(parts[0]).is_none()
                && view.resolve_nick(parts[1]).is_none()
            {
                return AskMatch::accept_say(
                    DetectorChoice::Center(a, b),
                    format!(
                        "📡 Locked in {} & {} — cards reveal when the phase ends!",
                        a.label(roster),
                        b.label(roster)
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
            format!(
                "Reply player <nick> (or <nick>), {}, or pass.",
                roster.center_pair_usage()
            )
            .yellow(),
        );
    };

    match view.resolve_nick(token) {
        Some(idx) if idx == actor_seat => {
            AskMatch::reject("You cannot scan your own card!".yellow())
        }
        Some(idx) if view.is_shielded(idx) => {
            AskMatch::reject("🛡️ That player's card has a Shield token!".yellow())
        }
        Some(idx) => AskMatch::accept_say(
            DetectorChoice::Player(idx),
            irc_format!(
                "📡 Locked in scanning {}'s card — reveals when the phase ends!",
                view.roster.seats[idx].user.nick().bold(),
            ),
        ),
        None => AskMatch::reject("Unknown player nickname.".yellow()),
    }
}

#[cfg(test)]
#[path = "detector_test.rs"]
mod test;

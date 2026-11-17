use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team,
    WakeStep,
};

/// Per-wake randomized Mother Ship mischief mode rolled for `#7F Rascal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RascalMode {
    /// Like `Village Idiot`: may shift all other unshielded players' cards left or right.
    Idiot,
    /// Like `Troublemaker`: may swap 2 other unshielded players' cards without viewing them.
    Troublemaker,
    /// Like `Drunk`: must swap own card with 1 center card without viewing it.
    Drunk,
    /// Like `Witch`: may view 1 center card; if so, must give it to any unshielded player.
    Witch,
    /// Like `Robber`: may swap own card with another unshielded player's card and view the new card.
    Robber,
}

impl RascalMode {
    fn roll() -> Self {
        match fastrand::u8(..5) {
            0 => Self::Idiot,
            1 => Self::Troublemaker,
            2 => Self::Drunk,
            3 => Self::Witch,
            _ => Self::Robber,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedRascalMode(pub Option<RascalMode>);

impl BoardKey for ForcedRascalMode {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy)]
enum RascalAction {
    Pass(&'static str),
    Rotate { right: bool },
    SwapTwo(usize, usize),
    DrunkCenter { slot: usize, was_random: bool },
    WitchSwap { slot: usize, target: usize },
    WitchBlocked { slot: usize },
    Rob(usize),
}

pub struct Rascal;
pub static RASCAL: Rascal = Rascal;

impl Role for Rascal {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "rascal",
            "Rascal",
            GameSet::Alien,
            Team::Village,
            "Wake to carry out a randomized mischief action from the Mother Ship (Idiot shift, Troublemaker swap, Drunk center swap, Witch center brew, or Robber steal)!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::group(&RASCAL, |seats, ctx| Box::pin(Self::wake(seats, ctx)))
                .run_when_unheld()
                .reads(BoardAccess::CENTER_FACES)
                .writes(
                    BoardAccess::PLAYER_FACES
                        | BoardAccess::CENTER_FACES
                        | BoardAccess::PUBLIC_ANNOUNCE,
                )]
        }
    }
}

impl Rascal {
    async fn wake(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let mode = ctx
            .get_key::<ForcedRascalMode>()
            .0
            .unwrap_or_else(RascalMode::roll);
        let effects = ctx
            .prompt_seats(&RASCAL, active_seats, |seat, pctx| {
                Box::pin(Self::prompt_seat(seat, mode, pctx))
            })
            .await?;

        Ok(GroupEffect::new(move |board, rctx| {
            rctx.apply_seat_effects(effects, board);
            let desc = match mode {
                RascalMode::Idiot => {
                    "shift all other players' cards left or right (Village Idiot mode)"
                }
                RascalMode::Troublemaker => "swap 2 other players' cards (Troublemaker mode)",
                RascalMode::Drunk => "exchange their own card with 1 center card (Drunk mode)",
                RascalMode::Witch => "view 1 center card and give it to a player (Witch mode)",
                RascalMode::Robber => "steal another player's card and view it (Robber mode)",
            };
            rctx.announce(irc_format!(
                "😜 {} {} was instructed to {desc}!",
                "Mother Ship (Rascal):".bold().cyan(),
                RASCAL.info().styled_name(),
            ));
        }))
    }

    async fn prompt_seat(
        seat: usize,
        mode: RascalMode,
        mut ctx: PromptCtx<'_>,
    ) -> Result<SeatEffect> {

        let action = match mode {
            RascalMode::Idiot => {
                let players_str = ctx.roster.format_players();
                let ans = ctx
                    .ask(
                        irc_format!(
                            "😜 Rascal (Idiot Mode): Reply {} ({}), {} ({}), or {} to shift all other unshielded cards! Players: {players_str}",
                            "left".bold(),
                            "-1".bold(),
                            "right".bold(),
                            "+1".bold(),
                            "pass".bold(),
                        ),
                        |m| {
                            if Roster::is_pass(&m.text) {
                                return AskMatch::accept_say(
                                    None,
                                    "😜 Passed — left all cards in place.",
                                );
                            }
                            match m.text.trim().to_ascii_lowercase().as_str() {
                                "left" | "l" | "-1" => AskMatch::accept_say(
                                    Some(false),
                                    "😜 Locked in shifting other cards LEFT!",
                                ),
                                "right" | "r" | "+1" => AskMatch::accept_say(
                                    Some(true),
                                    "😜 Locked in shifting other cards RIGHT!",
                                ),
                                _ => AskMatch::reject("Reply left, right, or pass.".yellow()),
                            }
                        },
                    )
                    .await?;
                match ans {
                    Answer::Ok(Some(right)) => RascalAction::Rotate { right },
                    _ => RascalAction::Pass("Idiot mode"),
                }
            }

            RascalMode::Troublemaker => {
                let players_str = ctx.roster.format_players();
                let view = ctx.view();
                let ans = ctx
                    .ask(
                        irc_format!(
                            "😜 Rascal (Troublemaker Mode): Reply with TWO other players ({}) to swap blindly, or {}. Players: {players_str}",
                            "<nick1> <nick2>".bold(),
                            "pass".bold(),
                        ),
                        move |m| {
                            if Roster::is_pass(&m.text) {
                                return AskMatch::accept_say(
                                    None,
                                    "😜 Passed — left all cards untouched.",
                                );
                            }
                            let parts: Vec<&str> = m.text.split_whitespace().collect();
                            if parts.len() != 2 {
                                return AskMatch::reject("Usage: <nick1> <nick2> or pass.".yellow());
                            }
                            let Some(a) = view.resolve_nick(parts[0]) else {
                                return AskMatch::reject(
                                    format!("Unknown player '{}'.", parts[0]).yellow(),
                                );
                            };
                            let Some(b) = view.resolve_nick(parts[1]) else {
                                return AskMatch::reject(
                                    format!("Unknown player '{}'.", parts[1]).yellow(),
                                );
                            };
                            if a == seat || b == seat || a == b {
                                return AskMatch::reject(
                                    "Pick TWO DIFFERENT players other than yourself!".yellow(),
                                );
                            }
                            if view.is_shielded(a) || view.is_shielded(b) {
                                return AskMatch::reject(
                                    "🛡️ Shielded players cannot have their cards swapped!".yellow(),
                                );
                            }
                            AskMatch::accept_say(
                                Some((a, b)),
                                irc_format!(
                                    "😜 Locked in swap between {} and {}!",
                                    view.roster.seats[a].user.nick().bold(),
                                    view.roster.seats[b].user.nick().bold(),
                                ),
                            )
                        },
                    )
                    .await?;
                match ans {
                    Answer::Ok(Some((a, b))) => RascalAction::SwapTwo(a, b),
                    _ => RascalAction::Pass("Troublemaker mode"),
                }
            }

            RascalMode::Drunk => {
                if ctx.is_shielded(seat) {
                    ctx.say(
                        "🛡️ Rascal (Drunk Mode): Your own card has a Sentinel Shield — you cannot swap it with the center!"
                            .yellow(),
                    );
                    RascalAction::Pass("Drunk mode, self shielded")
                } else {
                    let ans = ctx
                        .ask(
                            irc_format!(
                                "😜 Rascal (Drunk Mode): You MUST swap your card with a center card ({}, {}, or {}) without looking!",
                                "1".bold(),
                                "2".bold(),
                                "3".bold(),
                            ),
                            |m| match Roster::parse_center_slot(&m.text) {
                                Some(slot) => AskMatch::accept_say(
                                    slot,
                                    format!("😜 Locked in blind swap with Center #{}!", slot + 1),
                                ),
                                None => AskMatch::reject("Reply 1, 2, or 3.".yellow()),
                            },
                        )
                        .await?;
                    match ans {
                        Answer::Ok(slot) => RascalAction::DrunkCenter {
                            slot,
                            was_random: false,
                        },
                        _ => RascalAction::DrunkCenter {
                            slot: fastrand::usize(0..3),
                            was_random: true,
                        },
                    }
                }
            }

            RascalMode::Witch => {
                let step1 = ctx
                    .ask(
                        irc_format!(
                            "😜 Rascal (Witch Mode): Reply {}, {}, or {} to view a center card (you MUST then give it to someone!), or {}:",
                            "1".bold(),
                            "2".bold(),
                            "3".bold(),
                            "pass".bold(),
                        ),
                        |m| {
                            if Roster::is_pass(&m.text) {
                                return AskMatch::accept_say(None, "😜 Passed — left center untouched.");
                            }
                            match Roster::parse_center_slot(&m.text) {
                                Some(slot) => AskMatch::accept(Some(slot)),
                                None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
                            }
                        },
                    )
                    .await?;
                if let Answer::Ok(Some(slot)) = step1 {
                    let seen = ctx.peek_center_card(slot);
                    ctx.say(irc_format!(
                        "😜 Center #{} is {}!",
                        slot + 1,
                        seen.display_for_peek(),
                    ));
                    let unshielded = ctx.roster.all_seats() - ctx.shielded_seats();
                    if unshielded.is_empty() {
                        ctx.say("🛡️ All player cards are shielded!".yellow());
                        RascalAction::WitchBlocked { slot }
                    } else {
                        let players_str = ctx.roster.format_players();
                        let view = ctx.view();
                        let step2 = ctx
                            .ask(
                                irc_format!(
                                    "😜 Give Center #{} to which player ({}, including yourself)? Players: {players_str}",
                                    slot + 1,
                                    "<nick>".bold(),
                                ),
                                move |m| match view.resolve_nick(&m.text) {
                                    Some(idx) if view.is_shielded(idx) => {
                                        AskMatch::reject("🛡️ That player is shielded!".yellow())
                                    }
                                    Some(idx) => AskMatch::accept_say(
                                        idx,
                                        irc_format!(
                                            "😜 Swapped Center #{} onto {}!",
                                            slot + 1,
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
                                    "😜 Time expired — swapped Center #{} with {}!",
                                    slot + 1,
                                    ctx.nick(auto)
                                ));
                                auto
                            }
                        };
                        RascalAction::WitchSwap { slot, target }
                    }
                } else {
                    RascalAction::Pass("Witch mode")
                }
            }

            RascalMode::Robber => {
                if ctx.is_shielded(seat) {
                    ctx.say(
                        "🛡️ Rascal (Robber Mode): Your own card has a Sentinel Shield — you cannot rob anyone!"
                            .yellow(),
                    );
                    RascalAction::Pass("Robber mode, self shielded")
                } else {
                    let players_str = ctx.roster.format_players();
                    let view = ctx.view();
                    let ans = ctx
                        .ask(
                            irc_format!(
                                "😜 Rascal (Robber Mode): Reply {} to steal another player's card and view it, or {}. Players: {players_str}",
                                "<nick>".bold(),
                                "pass".bold(),
                            ),
                            move |m| {
                                if Roster::is_pass(&m.text) {
                                    return AskMatch::accept_say(
                                        None,
                                        "😜 Passed — stole nothing tonight.",
                                    );
                                }
                                match view.resolve_nick(&m.text) {
                                    Some(idx) if idx == seat => {
                                        AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                                    }
                                    Some(idx) if view.is_shielded(idx) => {
                                        AskMatch::reject("🛡️ That player's card is shielded!".yellow())
                                    }
                                    Some(idx) => AskMatch::accept_say(
                                        Some(idx),
                                        irc_format!(
                                            "😜 Locked in stealing {}'s card — reveals when the phase ends!",
                                            view.roster.seats[idx].user.nick().bold(),
                                        ),
                                    ),
                                    None => AskMatch::reject("Unknown player nickname (or pass).".yellow()),
                                }
                            },
                        )
                        .await?;
                    match ans {
                        Answer::Ok(Some(idx)) => RascalAction::Rob(idx),
                        _ => RascalAction::Pass("Robber mode"),
                    }
                }
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, action, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        action: RascalAction,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match action {
            RascalAction::Pass(mode_str) => {
                ctx.record(irc_format!(
                    "😜 {actor_label} ({}) passed ({mode_str}).",
                    actor_nick.bold(),
                ));
            }
            RascalAction::Rotate { right } => {
                let moved = board.rotate_other_unshielded(seat, right);
                if moved >= 2 {
                    let dir = if right { "right" } else { "left" };
                    ctx.record(irc_format!(
                        "😜 {actor_label} ({}) shifted {moved} unshielded cards {dir}.",
                        actor_nick.bold(),
                    ));
                }
            }
            RascalAction::SwapTwo(a, b) => {
                if let Some((ca, cb)) = board.swap_players(seat, a, b) {
                    ctx.record(irc_format!(
                        "😜 {actor_label} ({}) swapped {} ({}) and {} ({}).",
                        actor_nick.bold(),
                        ctx.nick(a).bold(),
                        ca.display_for_peek(),
                        ctx.nick(b).bold(),
                        cb.display_for_peek(),
                    ));
                }
            }
            RascalAction::DrunkCenter { slot, was_random } => {
                if was_random {
                    ctx.dm(
                        seat,
                        format!("🎲 Randomly swapped your card with Center #{}.", slot + 1),
                    );
                }
                if let Some((old_p, new_p)) = board.swap_player_with_center(seat, seat, slot) {
                    ctx.record(irc_format!(
                        "😜 {actor_label} ({}) blindly swapped {} with Center #{} ({}).",
                        actor_nick.bold(),
                        old_p.display_for_peek(),
                        slot + 1,
                        new_p.display_for_peek(),
                    ));
                }
            }
            RascalAction::WitchBlocked { slot } => {
                board.record_prompt_peek(seat);
                ctx.record(irc_format!(
                    "😜 {actor_label} ({}) peeked Center #{}, but all players were shielded.",
                    actor_nick.bold(),
                    slot + 1,
                ));
            }
            RascalAction::WitchSwap { slot, target } => {
                if let Some((old_p, center_c)) = board.swap_player_with_center(seat, target, slot) {
                    ctx.record(irc_format!(
                        "😜 {actor_label} ({}) peeked Center #{} ({}) and swapped it onto {} (moving {} to Center #{}).",
                        actor_nick.bold(),
                        slot + 1,
                        center_c.display_for_peek(),
                        ctx.nick(target).bold(),
                        old_p.display_for_peek(),
                        slot + 1,
                    ));
                }
            }
            RascalAction::Rob(target) => {
                if let Some((given, stolen)) = board.swap_players(seat, seat, target) {
                    let target_nick = ctx.nick(target);
                    ctx.dm(
                        seat,
                        irc_format!(
                            "😜 You stole {}'s card — your new role is {}!",
                            target_nick.as_str().bold(),
                            stolen.display_for_peek(),
                        ),
                    );
                    ctx.record(irc_format!(
                        "😜 {actor_label} ({}) robbed {} ({} ↔ {}).",
                        actor_nick.bold(),
                        target_nick.bold(),
                        given.display_for_peek(),
                        stolen.display_for_peek(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "rascal_test.rs"]
mod test;

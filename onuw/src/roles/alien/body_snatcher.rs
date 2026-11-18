use ircgm::prelude::*;

use super::alien::ALIEN_PACK_STEP;
use crate::board::{BoardAccess, BoardKey, BoardMut, CenterSlot, Roster};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodySnatcherMode {
    /// Must steal an unshielded non-Alien player's card, viewing it and turning it into an Alien!
    StealPlayer,
    /// Must swap own card with 1 center card, viewing the new card and turning it into an Alien!
    StealCenter,
    /// Mother Ship orders Body Snatcher to pretend to move cards while doing nothing!
    Pretend,
}

impl BodySnatcherMode {
    fn roll() -> Self {
        match fastrand::u8(..100) {
            0..=54 => Self::StealPlayer,
            55..=84 => Self::StealCenter,
            _ => Self::Pretend,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedBodySnatcherMode(pub Option<BodySnatcherMode>);

impl BoardKey for ForcedBodySnatcherMode {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveBodySnatcherMode(BodySnatcherMode);

impl BoardKey for ActiveBodySnatcherMode {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(BodySnatcherMode::Pretend)
    }
}

struct SnatchedAlien;
static SNATCHED_ALIEN: SnatchedAlien = SnatchedAlien;

impl Role for SnatchedAlien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "snatched_alien",
            "Alien",
            GameSet::Alien,
            Team::Alien,
            "Card assimilated into the Alien team by the Body Snatcher.",
        )
        .night_faction(NightFaction::Alien)
    }
}

#[derive(Clone, Copy)]
enum SnatchAction {
    Pretend,
    Blocked(&'static str),
    Player { target: usize, was_random: bool },
    Center { slot: CenterSlot, was_random: bool },
}

pub struct BodySnatcher;
pub static BODY_SNATCHER: BodySnatcher = BodySnatcher;

const BODY_SNATCHER_SOLO_STEP: WakeStep =
    WakeStep::solo(&BODY_SNATCHER, |seat, ctx| {
        Box::pin(BodySnatcher::prompt_seat(seat, ctx))
    })
    .copy_on_turn()
    .with_tier_hooks(BodySnatcher::init_tier, BodySnatcher::finish_tier)
    .reads(BoardAccess::PLAYER_FACES)
    .writes(
        BoardAccess::PLAYER_FACES
            | BoardAccess::CENTER_FACES
            | BoardAccess::PLAYER_BECAME
            | BoardAccess::PUBLIC_ANNOUNCE,
    );

impl Role for BodySnatcher {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "body_snatcher",
            "Body Snatcher",
            GameSet::Alien,
            Team::Alien,
            "Wake with the Aliens, then wake solo: steal a non-Alien player's card (or a center card) and turn your new card into an Alien — or pretend to move cards!",
        )
        .aliases(&["bodysnatcher", "snatcher"])
        .night_faction(NightFaction::Alien)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP, BODY_SNATCHER_SOLO_STEP] }
    }
}

impl BodySnatcher {
    fn init_tier(board: &mut BoardMut<'_>, _roster: &Roster) {
        let mode = board
            .get_key::<ForcedBodySnatcherMode>()
            .0
            .unwrap_or_else(BodySnatcherMode::roll);
        board.set_key(ActiveBodySnatcherMode(mode));
    }

    fn finish_tier(board: &mut BoardMut<'_>, rctx: &mut ResolveCtx<'_>) {
        let mode = board.get_key::<ActiveBodySnatcherMode>().0;
        let msg = match mode {
            BodySnatcherMode::StealPlayer => {
                "steal a non-Alien player's card (assimilating it into an Alien)!"
            }
            BodySnatcherMode::StealCenter => {
                "steal 1 center card (assimilating it into an Alien)!"
            }
            BodySnatcherMode::Pretend => {
                "PRETEND to move cards around without actually moving anything!"
            }
        };
        rctx.announce(irc_format!(
            "🛸🧬 {} {} was instructed to {msg}",
            "Mother Ship (Body Snatcher):".bold().green(),
            BODY_SNATCHER.info().styled_name(),
        ));
    }

    async fn prompt_seat(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let mode = ctx.get_key::<ActiveBodySnatcherMode>().0;

        let action = match mode {
            BodySnatcherMode::Pretend => {
                ctx.say(
                    "🛸🧬 Body Snatcher: Mother Ship orders you to PRETEND to move cards tonight — your card stays put!"
                        .yellow(),
                );
                SnatchAction::Pretend
            }

            BodySnatcherMode::StealPlayer => {
                if ctx.is_shielded(seat) {
                    ctx.say(
                        "🛡️ Body Snatcher: Your own card has a Sentinel Shield — you cannot swap it!"
                            .yellow(),
                    );
                    SnatchAction::Blocked("self shielded")
                } else {
                    let aliens = ctx.known_pack_mates(NightFaction::Alien);
                    let eligible =
                        (ctx.roster.all_seats() - aliens - ctx.shielded_seats()).without(seat);
                    if eligible.is_empty() {
                        ctx.say(
                            "🛸🧬 Body Snatcher: No unshielded non-Alien players are available to snatch!"
                                .yellow(),
                        );
                        SnatchAction::Blocked("no eligible non-Alien targets")
                    } else {
                        let targets_str = ctx.roster.format_seats(eligible);
                        let view = ctx.view();
                        let ans = ctx
                            .ask(
                                irc_format!(
                                    "🛸🧬 Body Snatcher: You MUST steal a non-Alien player's card (turning your new card into an Alien while giving them your Alien card)! Reply {}. Eligible: {targets_str}",
                                    "<nick>".bold(),
                                ),
                                move |m| match view.resolve_nick(&m.text) {
                                    Some(idx) if idx == seat => {
                                        AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                                    }
                                    Some(idx) if view.is_shielded(idx) => {
                                        AskMatch::reject("🛡️ That player's card is shielded!".yellow())
                                    }
                                    Some(idx) if !eligible.contains(idx) => {
                                        AskMatch::reject("You cannot snatch a fellow Alien!".yellow())
                                    }
                                    Some(idx) => AskMatch::accept_say(
                                        idx,
                                        irc_format!(
                                            "🛸🧬 Locked in snatching {}'s body — reveals when the phase ends!",
                                            view.roster.seats[idx].user.nick().bold(),
                                        ),
                                    ),
                                    None => AskMatch::reject("Unknown player nickname.".yellow()),
                                },
                            )
                            .await?;
                        match ans {
                            Answer::Ok(target) => SnatchAction::Player {
                                target,
                                was_random: false,
                            },
                            _ => SnatchAction::Player {
                                target: eligible.choose_random().expect("eligible non-empty"),
                                was_random: true,
                            },
                        }
                    }
                }
            }

            BodySnatcherMode::StealCenter => {
                if ctx.is_shielded(seat) {
                    ctx.say(
                        "🛡️ Body Snatcher: Your own card has a Sentinel Shield — you cannot swap it!"
                            .yellow(),
                    );
                    SnatchAction::Blocked("self shielded")
                } else {
                    let roster = ctx.roster;
                    let hint = roster.center_choice_hint();
                    let ans = ctx
                        .ask(
                            irc_format!(
                                "🛸🧬 Body Snatcher: You MUST steal a center card ({}), viewing it and turning it into an Alien!",
                                hint.as_str().bold(),
                            ),
                            move |m| match roster.parse_center_slot(&m.text) {
                                Some(slot) => AskMatch::accept_say(
                                    slot,
                                    format!(
                                        "🛸🧬 Locked in snatching {} — reveals when the phase ends!",
                                        slot.label(roster)
                                    ),
                                ),
                                None => AskMatch::reject(format!("Reply {hint}.").yellow()),
                            },
                        )
                        .await?;
                    match ans {
                        Answer::Ok(slot) => SnatchAction::Center {
                            slot,
                            was_random: false,
                        },
                        _ => SnatchAction::Center {
                            slot: roster.choose_random_center_slot(),
                            was_random: true,
                        },
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
        action: SnatchAction,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match action {
            SnatchAction::Pretend => {
                ctx.record(irc_format!(
                    "🛸🧬 {actor_label} ({}) pretended to move cards.",
                    actor_nick.bold(),
                ));
            }
            SnatchAction::Blocked(why) => {
                ctx.record(irc_format!(
                    "🛸🧬 {actor_label} ({}) could not snatch ({why}).",
                    actor_nick.bold(),
                ));
            }
            SnatchAction::Player { target, was_random } => {
                if was_random {
                    ctx.dm(
                        seat,
                        format!("🎲 Randomly snatched {}'s card.", ctx.nick(target)),
                    );
                }
                if let Some((given_card, stolen_card)) = board.swap_players(seat, seat, target) {
                    board.transform_in_place(seat, &SNATCHED_ALIEN);
                    let target_nick = ctx.nick(target);
                    ctx.dm(
                        seat,
                        irc_format!(
                            "🛸🧬 You snatched {}'s {} (giving them {}) — your stolen card is now an {} too!",
                            target_nick.as_str().bold(),
                            stolen_card.display_for_peek(),
                            given_card.display_for_peek(),
                            SNATCHED_ALIEN.info().styled_name(),
                        ),
                    );
                    ctx.record(irc_format!(
                        "🛸🧬 {actor_label} ({}) snatched {}'s {} (giving them {}) and assimilated it into an {}!",
                        actor_nick.bold(),
                        target_nick.bold(),
                        stolen_card.display_for_peek(),
                        given_card.display_for_peek(),
                        SNATCHED_ALIEN.info().styled_name(),
                    ));
                }
            }
            SnatchAction::Center { slot, was_random } => {
                let slot_label = slot.label(ctx.roster);
                if was_random {
                    ctx.dm(seat, format!("🎲 Randomly snatched {slot_label}."));
                }
                if let Some((given_card, stolen_center)) =
                    board.swap_player_with_center(seat, seat, slot)
                {
                    board.transform_in_place(seat, &SNATCHED_ALIEN);
                    ctx.dm(
                        seat,
                        irc_format!(
                            "🛸🧬 {slot_label} was {} (swapped with your {}) — your new card is now an {}!",
                            stolen_center.display_for_peek(),
                            given_card.display_for_peek(),
                            SNATCHED_ALIEN.info().styled_name(),
                        ),
                    );
                    ctx.record(irc_format!(
                        "🛸🧬 {actor_label} ({}) snatched {slot_label} ({}, leaving {}) and assimilated it into an {}!",
                        actor_nick.bold(),
                        stolen_center.display_for_peek(),
                        given_card.display_for_peek(),
                        SNATCHED_ALIEN.info().styled_name(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "body_snatcher_test.rs"]
mod test;

use ircgm::prelude::*;

use super::oracle::OracleAlienExchange;
use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::pointing::{parse_single_player_point, silent_point};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, SpecialVictory,
    Team, VictoryCtx, WakeStep,
};

pub struct Alien;
pub static ALIEN: Alien = Alien;

pub(crate) const ALIEN_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Alien,
    |seats, ctx| Box::pin(Alien::wake_group(seats, ctx)),
)
.run_when_unheld()
.reads(BoardAccess::CUSTOM_0)
.writes(BoardAccess::PLAYER_FACES | BoardAccess::PUBLIC_ANNOUNCE);

/// Per-game randomized Mother Ship event rolled for `#1A Aliens`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlienEvent {
    /// Standard ONUA Alien wake (`Stare`): recognize fellow Aliens (no solo center peek).
    StandardRecon,
    /// Mother Ship Bio-Scan (`View Player`): view 1 unshielded non-Alien player's card together.
    BioScan,
    /// Mother Ship Left Rotation (`Left`): each unshielded waking Alien passes their card to the waking Alien on their left.
    PassLeft,
    /// Mother Ship Right Rotation (`Right`): each unshielded waking Alien passes their card to the waking Alien on their right.
    PassRight,
    /// Mother Ship Center Probe (`View Center`): view 1 center card together.
    CenterProbe,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedAlienEvent(pub Option<AlienEvent>);

impl BoardKey for ForcedAlienEvent {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AlienPointChoice {
    Pass,
    Scan(usize),
    Probe(usize),
}

enum AlienDirective {
    None,
    ForcedOracleExchange,
    RotateCards { pass_right: bool },
    Scan { actor: usize, target: usize },
    CenterProbe { actor: usize, slot: usize },
}

impl Role for Alien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "alien",
            "Alien",
            GameSet::Alien,
            Team::Alien,
            "Wake with the Aliens to recognize each other and carry out the rolled Mother Ship event (recon, bio-scan, pass Alien cards left/right, or center probe)!",
        )
        .night_faction(NightFaction::Alien)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP] }
    }
}

impl Alien {
    fn event_announcement(event: Option<AlienEvent>) -> Text {
        match event {
            None => irc_format!(
                "🛸 {} The {} were ordered by the Oracle to exchange cards!",
                "Mother Ship (Aliens):".bold().green(),
                Team::Alien.style_text("Aliens"),
            ),
            Some(AlienEvent::StandardRecon) => irc_format!(
                "🛸 {} {} woke to look for fellow Aliens!",
                "Mother Ship (Aliens):".bold().green(),
                Team::Alien.style_text("Aliens"),
            ),
            Some(AlienEvent::BioScan) => irc_format!(
                "🛸 {} {} were instructed to bio-scan 1 non-Alien player's card!",
                "Mother Ship (Aliens):".bold().green(),
                Team::Alien.style_text("Aliens"),
            ),
            Some(AlienEvent::PassLeft) => irc_format!(
                "🛸 {} {} were instructed to pass their Alien cards to the Alien on their LEFT!",
                "Mother Ship (Aliens):".bold().green(),
                Team::Alien.style_text("Aliens"),
            ),
            Some(AlienEvent::PassRight) => irc_format!(
                "🛸 {} {} were instructed to pass their Alien cards to the Alien on their RIGHT!",
                "Mother Ship (Aliens):".bold().green(),
                Team::Alien.style_text("Aliens"),
            ),
            Some(AlienEvent::CenterProbe) => irc_format!(
                "🛸 {} {} were instructed to probe 1 center card together!",
                "Mother Ship (Aliens):".bold().green(),
                Team::Alien.style_text("Aliens"),
            ),
        }
    }

    fn roll_event() -> AlienEvent {
        match fastrand::u8(..100) {
            0..=39 => AlienEvent::StandardRecon,
            40..=64 => AlienEvent::BioScan,
            65..=74 => AlienEvent::PassLeft,
            75..=84 => AlienEvent::PassRight,
            _ => AlienEvent::CenterProbe,
        }
    }

    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let visible_aliens = ctx.waking_seats_with_faction(NightFaction::Alien);
        let alien_list = ctx.roster.format_seats(visible_aliens);
        let cow_seats = ctx.waking_seats_with_role(&super::cow::COW);
        let cow_list = ctx.roster.format_seats(cow_seats);

        for s in active_seats {
            ctx.dm(
                s,
                irc_format!(
                    "👽 Mother Ship Scan — Aliens aboard: {}",
                    alien_list.as_str().bold().green(),
                ),
            );
            if !cow_seats.is_empty() {
                ctx.dm(
                    s,
                    irc_format!(
                        "🐄🛸 Alien Hoof-Scan: {} holds the Cow card!",
                        cow_list.as_str().bold().yellow(),
                    ),
                );
            }
            if ctx.waking_roles[s].id() == "synthetic_alien" {
                ctx.dm(
                    s,
                    "🤖👽 Synthetic Alien: You blended into the Alien scan! Get YOURSELF killed in the vote to win!",
                );
            }
        }

        let shielded = ctx.shielded_seats();

        if ctx.get_key::<OracleAlienExchange>().0 {
            let unshielded_active = active_seats - shielded;
            if unshielded_active.len() >= 2 {
                for s in unshielded_active {
                    ctx.dm(
                        s,
                        "🛸 Mother Ship Override (Oracle): Your Alien cards will exchange places at the end of this phase!",
                    );
                }
                for s in active_seats - unshielded_active {
                    ctx.dm(
                        s,
                        "🛡️🛸 Mother Ship Override (Oracle): Your Shield blocks your Alien card from being exchanged!",
                    );
                }
            } else {
                for s in active_seats {
                    ctx.dm(
                        s,
                        "🛸 Mother Ship Override (Oracle): Fewer than 2 unshielded Aliens woke — no cards can be exchanged!",
                    );
                }
            }
            return Ok(GroupEffect::new(move |board, rctx| {
                Self::resolve(
                    active_seats,
                    None,
                    AlienDirective::ForcedOracleExchange,
                    board,
                    rctx,
                );
            }));
        }

        let non_alien_seats = ctx.roster.all_seats() - visible_aliens;
        let non_aliens = non_alien_seats - shielded;

        let rolled_event = ctx
            .get_key::<ForcedAlienEvent>()
            .0
            .unwrap_or_else(Self::roll_event);

        if active_seats.is_empty() {
            return Ok(GroupEffect::new(move |board, rctx| {
                Self::resolve(
                    active_seats,
                    Some(rolled_event),
                    AlienDirective::None,
                    board,
                    rctx,
                );
            }));
        }

        let directive = match rolled_event {
            AlienEvent::StandardRecon => AlienDirective::None,
            AlienEvent::PassLeft | AlienEvent::PassRight => {
                let pass_right = rolled_event == AlienEvent::PassRight;
                let dir_word = if pass_right { "RIGHT" } else { "LEFT" };
                let unshielded_active = active_seats - shielded;
                if unshielded_active.len() >= 2 {
                    for s in unshielded_active {
                        ctx.dm(
                            s,
                            format!(
                                "🛸 Mother Ship Rotation ({dir_word}): Your Alien card will pass to the unshielded waking Alien on your {dir_word} at the end of this phase!"
                            ),
                        );
                    }
                    for s in active_seats - unshielded_active {
                        ctx.dm(
                            s,
                            format!(
                                "🛡️🛸 Mother Ship Rotation ({dir_word}): Your Shield blocks your Alien card from rotating!"
                            ),
                        );
                    }
                    AlienDirective::RotateCards { pass_right }
                } else {
                    for s in active_seats {
                        ctx.dm(
                            s,
                            format!(
                                "🛸 Mother Ship Rotation ({dir_word}): Fewer than 2 unshielded Aliens woke — no Alien cards move tonight!"
                            ),
                        );
                    }
                    AlienDirective::None
                }
            }
            AlienEvent::BioScan if non_aliens.is_empty() => {
                for s in active_seats {
                    ctx.dm(
                        s,
                        "👽 Mother Ship Bio-Scan: No unshielded non-Alien players available to scan!"
                            .yellow(),
                    );
                }
                AlienDirective::None
            }
            ev => {
                Self::ask_pack_directive(
                    &ctx,
                    active_seats,
                    ev,
                    non_alien_seats,
                    non_aliens,
                    shielded,
                )
                .await?
            }
        };

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(active_seats, Some(rolled_event), directive, board, rctx);
        }))
    }

    async fn ask_pack_directive(
        ctx: &GroupPromptCtx<'_>,
        active_seats: SeatSet,
        event: AlienEvent,
        non_alien_seats: SeatSet,
        non_aliens: SeatSet,
        shielded: SeatSet,
    ) -> Result<AlienDirective> {
        let roster = ctx.roster;
        let non_aliens_str = ctx.roster.format_seats(non_alien_seats);

        let picked = silent_point(
            ctx,
            active_seats,
            |_seat| match event {
                AlienEvent::BioScan => irc_format!(
                    "👽 Mother Ship Bio-Scan — Reply {} (or {}) to point silently at a non-Alien! Non-Aliens: {non_aliens_str}",
                    "<nick>".bold(),
                    "pass".bold(),
                ),
                AlienEvent::CenterProbe
                | AlienEvent::StandardRecon
                | AlienEvent::PassLeft
                | AlienEvent::PassRight => irc_format!(
                    "🛸 Mother Ship Center Probe — Reply {} (or {}) to point silently at 1 center card!",
                    "1 / 2 / 3".bold(),
                    "pass".bold(),
                ),
            },
            |_seat, raw| {
                if Roster::is_pass(raw) {
                    return Ok(("waves pass!".to_owned(), Some(AlienPointChoice::Pass)));
                }
                match event {
                    AlienEvent::BioScan => parse_single_player_point(
                        roster,
                        raw,
                        "scan",
                        non_aliens,
                        shielded,
                        true,
                        "Pick a non-Alien player!",
                        "🤫 Silent night (no talking)! Reply <nick> or pass.",
                    )
                    .map(|(desc, opt)| (desc, opt.map(AlienPointChoice::Scan))),
                    AlienEvent::CenterProbe
                    | AlienEvent::StandardRecon
                    | AlienEvent::PassLeft
                    | AlienEvent::PassRight => {
                        let stripped = raw.strip_prefix('!').unwrap_or(raw).trim();
                        let parts: Vec<&str> = stripped.split_whitespace().collect();
                        let arg = match parts.as_slice() {
                            [w, slot]
                                if w.eq_ignore_ascii_case("probe")
                                    || w.eq_ignore_ascii_case("center") =>
                            {
                                *slot
                            }
                            _ => stripped,
                        };
                        match Roster::parse_center_slot(arg) {
                            Some(slot) => Ok((
                                format!("points at Center #{}!", slot + 1),
                                Some(AlienPointChoice::Probe(slot)),
                            )),
                            None => Err(
                                "🤫 Silent night (no talking)! Reply 1, 2, 3, or pass."
                                    .yellow()
                                    .into(),
                            ),
                        }
                    }
                }
            },
        )
        .await?;

        Ok(match picked {
            None | Some((_, AlienPointChoice::Pass)) => AlienDirective::None,
            Some((actor, AlienPointChoice::Scan(target))) => {
                AlienDirective::Scan { actor, target }
            }
            Some((actor, AlienPointChoice::Probe(slot))) => {
                AlienDirective::CenterProbe { actor, slot }
            }
        })
    }

    fn resolve(
        active_seats: SeatSet,
        rolled_event: Option<AlienEvent>,
        action: AlienDirective,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        ctx.announce(Self::event_announcement(rolled_event));
        if active_seats.is_empty() {
            ctx.record("👽 Aliens were all in the center (Mother Ship event announced).");
            return;
        }
        match action {
            AlienDirective::ForcedOracleExchange => {
                let unshielded: Vec<usize> = active_seats
                    .into_iter()
                    .filter(|&s| !board.is_shielded(s))
                    .collect();
                if unshielded.len() >= 2 {
                    for i in 0..unshielded.len() - 1 {
                        let _ = board.swap_players(unshielded[0], unshielded[i], unshielded[i + 1]);
                    }
                    let moved_set: SeatSet = unshielded.iter().copied().collect();
                    board.record_card_viewers(moved_set);
                    let names = ctx.roster.format_seats(moved_set);
                    for &s in &unshielded {
                        ctx.dm(
                            s,
                            irc_format!(
                                "🛸 Oracle Alien Exchange: Waking Aliens ({}) exchanged cards!",
                                names.as_str().bold().green(),
                            ),
                        );
                    }
                    ctx.record(irc_format!(
                        "🛸 Aliens ({}) exchanged cards per the Oracle's decree.",
                        names.bold(),
                    ));
                    return;
                }
            }
            AlienDirective::RotateCards { pass_right } => {
                let unshielded: Vec<usize> = active_seats
                    .into_iter()
                    .filter(|&s| !board.is_shielded(s))
                    .collect();
                if unshielded.len() >= 2 {
                    let k = unshielded.len();
                    if pass_right {
                        for i in (1..k).rev() {
                            let _ =
                                board.swap_players(unshielded[0], unshielded[i], unshielded[i - 1]);
                        }
                    } else {
                        for i in 0..k - 1 {
                            let _ =
                                board.swap_players(unshielded[0], unshielded[i], unshielded[i + 1]);
                        }
                    }
                    let moved_set: SeatSet = unshielded.iter().copied().collect();
                    board.record_card_viewers(moved_set);
                    let dir_word = if pass_right { "RIGHT" } else { "LEFT" };
                    for (pos, &s) in unshielded.iter().enumerate() {
                        let (to_seat, from_seat) = if pass_right {
                            (unshielded[(pos + 1) % k], unshielded[(pos + k - 1) % k])
                        } else {
                            (unshielded[(pos + k - 1) % k], unshielded[(pos + 1) % k])
                        };
                        ctx.dm(
                            s,
                            irc_format!(
                                "🛸 Mother Ship Rotation ({dir_word}): You passed your Alien card to {} and received {}'s Alien card!",
                                ctx.nick(to_seat).bold().green(),
                                ctx.nick(from_seat).bold().green(),
                            ),
                        );
                    }
                    let names = ctx.roster.format_seats(moved_set);
                    ctx.record(irc_format!(
                        "🛸 Aliens ({}) passed their Alien cards to the {dir_word}.",
                        names.bold(),
                    ));
                    return;
                }
            }
            AlienDirective::Scan { actor, target } => {
                if let Some(seen) = board.peek_player(actor, target) {
                    board.record_card_viewers(active_seats);
                    let target_nick = ctx.nick(target);
                    for s in active_seats {
                        ctx.dm(
                            s,
                            irc_format!(
                                "👽 Bio-Scan Result: {} holds {}!",
                                target_nick.as_str().bold(),
                                seen.display_for_peek(),
                            ),
                        );
                    }
                    ctx.record(irc_format!(
                        "👽 {} ({}) bio-scanned {} and saw {}.",
                        "Alien".bold().red(),
                        ctx.nick(actor).bold(),
                        target_nick.bold(),
                        seen.display_for_peek(),
                    ));
                    return;
                }
            }
            AlienDirective::CenterProbe { actor, slot } => {
                let seen = board.peek_center(actor, slot);
                board.record_card_viewers(active_seats);
                for s in active_seats {
                    ctx.dm(
                        s,
                        irc_format!(
                            "🛸 Mother Ship Center #{}: {}",
                            slot + 1,
                            seen.display_for_peek(),
                        ),
                    );
                }
                ctx.record(irc_format!(
                    "🛸 Aliens ({}) probed Center #{} ({}).",
                    ctx.roster.format_seats(active_seats).bold(),
                    slot + 1,
                    seen.display_for_peek(),
                ));
                return;
            }
            AlienDirective::None => {}
        }
        let pack_names = ctx.roster.format_seats(active_seats);
        ctx.record(irc_format!(
            "👽 Aliens ({}) woke aboard the Mother Ship.",
            pack_names.bold(),
        ));
    }
}

pub struct SyntheticAlien;
pub static SYNTHETIC_ALIEN: SyntheticAlien = SyntheticAlien;

impl Role for SyntheticAlien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "synthetic_alien",
            "Synthetic Alien",
            GameSet::Alien,
            Team::Independent,
            "Wake with the Aliens and appear Alien to Leader & Cow, but you ONLY win if YOU are killed!",
        )
        .aliases(&["synth", "synthalien"])
        .night_faction(NightFaction::Alien)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP] }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        ctx.is_dead(seat)
            .then_some(SpecialVictory::independent_sole_winner())
    }
}

#[cfg(test)]
#[path = "alien_test.rs"]
mod test;

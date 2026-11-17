use ircgm::prelude::*;

use super::oracle::OracleAlienExchange;
use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::pointing::{
    parse_single_player_point, parse_two_player_point, silent_point, SeatPair,
};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, SpecialVictory,
    Team, VictoryCtx, WakeStep,
};

pub struct Alien;
pub static ALIEN: Alien = Alien;

pub(crate) const ALIEN_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Alien,
    110,
    |seats, ctx| Box::pin(Alien::wake_group(seats, ctx)),
)
.reads(BoardAccess::CUSTOM_0)
.writes(BoardAccess::PLAYER_FACES);

/// Per-game randomized Mother Ship event rolled for `#1A Aliens`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlienEvent {
    /// Standard ONUA Alien wake: recognize fellow Aliens, or Lone Alien 1-center peek.
    StandardRecon,
    /// Mother Ship Bio-Scan: view 1 unshielded non-Alien player's card together.
    BioScan,
    /// Mother Ship Tractor Beam: swap 2 unshielded non-Alien players' cards.
    TractorBeam,
    /// Mother Ship Center Probe: view 1 center card together.
    CenterProbe,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedAlienEvent(pub Option<AlienEvent>);

impl BoardKey for ForcedAlienEvent {
    const ACCESS: u16 = BoardAccess::CUSTOM_0;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AlienPointChoice {
    Pass,
    Scan(usize),
    Beam(SeatPair),
    Probe(usize),
}

enum AlienDirective {
    None,
    ForcedOracleExchange,
    Scan { actor: usize, target: usize },
    TractorBeam { actor: usize, a: usize, b: usize },
    CenterProbe { actor: usize, slot: usize, lone: bool },
}

impl Role for Alien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "alien",
            "Alien",
            GameSet::Alien,
            Team::Alien,
            "Wake with the Aliens to recognize each other and carry out the rolled Mother Ship event (recon, bio-scan, tractor beam, or center probe)!",
        )
        .night_faction(NightFaction::Alien)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP] }
    }
}

impl Alien {
    fn roll_event() -> AlienEvent {
        match fastrand::u8(..100) {
            0..=39 => AlienEvent::StandardRecon,
            40..=64 => AlienEvent::BioScan,
            65..=84 => AlienEvent::TractorBeam,
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

        if ctx.get_key::<OracleAlienExchange>().0 {
            if active_seats.len() >= 2 {
                for s in active_seats {
                    ctx.dm(
                        s,
                        "🛸 Mother Ship Override (Oracle): Your Alien cards will exchange places at the end of this phase!",
                    );
                }
            } else if let Some(solo) = active_seats.only() {
                ctx.dm(
                    solo,
                    "🛸 Mother Ship Override (Oracle): No other Alien woke to exchange cards with!",
                );
            }
            return Ok(GroupEffect::new(move |board, rctx| {
                Self::resolve(
                    active_seats,
                    AlienDirective::ForcedOracleExchange,
                    board,
                    rctx,
                );
            }));
        }

        let shielded = ctx.shielded_seats();
        let non_alien_seats = ctx.roster.all_seats() - visible_aliens;
        let non_aliens = non_alien_seats - shielded;

        let mut event = ctx
            .get_key::<ForcedAlienEvent>()
            .0
            .unwrap_or_else(Self::roll_event);
        if event == AlienEvent::BioScan && non_aliens.is_empty() {
            event = AlienEvent::CenterProbe;
        }
        if event == AlienEvent::TractorBeam && non_aliens.len() < 2 {
            event = if !non_aliens.is_empty() {
                AlienEvent::BioScan
            } else {
                AlienEvent::CenterProbe
            };
        }

        let directive = match (event, active_seats.only()) {
            (AlienEvent::StandardRecon, None) => AlienDirective::None,
            (AlienEvent::StandardRecon, Some(solo)) => {
                let prompt = irc_format!(
                    "👽 Lone Alien: Reply {}, {}, {}, or {} to peek at ONE center card!",
                    "1".bold(),
                    "2".bold(),
                    "3".bold(),
                    "pass".bold(),
                );
                let ans = ctx
                    .ask(solo, prompt, |m| {
                        if Roster::is_pass(&m.text) {
                            return AskMatch::accept_say(
                                AlienDirective::None,
                                "👽 Passed on peeking the center.",
                            );
                        }
                        match Roster::parse_center_slot(&m.text) {
                            Some(slot) => AskMatch::accept_say(
                                AlienDirective::CenterProbe {
                                    actor: solo,
                                    slot,
                                    lone: true,
                                },
                                format!(
                                    "👽 Locked in Center #{} — reveals when the phase ends!",
                                    slot + 1
                                ),
                            ),
                            None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
                        }
                    })
                    .await?;
                ans.ok().unwrap_or(AlienDirective::None)
            }
            (ev, _) => {
                Self::ask_pack_directive(&ctx, active_seats, ev, non_alien_seats, non_aliens, shielded)
                    .await?
            }
        };

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(active_seats, directive, board, rctx);
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
        let non_aliens_str = ctx.format_visible_seats(non_alien_seats);

        let picked = silent_point(
            ctx,
            active_seats,
            |_seat| match event {
                AlienEvent::BioScan => irc_format!(
                    "👽 Mother Ship Bio-Scan — Reply {} (or {}) to point silently at a non-Alien! Non-Aliens: {non_aliens_str}",
                    "<nick>".bold(),
                    "pass".bold(),
                ),
                AlienEvent::TractorBeam => irc_format!(
                    "🛸 Mother Ship Tractor Beam — Reply {} (or {}) to point silently at two non-Aliens to swap! Non-Aliens: {non_aliens_str}",
                    "<nick1> <nick2>".bold(),
                    "pass".bold(),
                ),
                AlienEvent::CenterProbe | AlienEvent::StandardRecon => irc_format!(
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
                    AlienEvent::TractorBeam => parse_two_player_point(
                        roster,
                        raw,
                        non_aliens,
                        shielded,
                        true,
                        "Pick two non-Alien players!",
                        "🤫 Silent night (no talking)! Reply <nick1> <nick2> or pass.",
                    )
                    .map(|(desc, opt)| (desc, opt.map(AlienPointChoice::Beam))),
                    AlienEvent::CenterProbe | AlienEvent::StandardRecon => {
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
            Some((actor, AlienPointChoice::Beam(pair))) => {
                let (a, b) = pair.seats();
                AlienDirective::TractorBeam { actor, a, b }
            }
            Some((actor, AlienPointChoice::Probe(slot))) => AlienDirective::CenterProbe {
                actor,
                slot,
                lone: false,
            },
        })
    }

    fn resolve(
        active_seats: SeatSet,
        action: AlienDirective,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
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
            AlienDirective::TractorBeam { actor, a, b } => {
                if let Some((card_a, card_b)) = board.swap_players(actor, a, b) {
                    board.record_card_viewers(active_seats);
                    for s in active_seats.without(actor) {
                        ctx.dm(
                            s,
                            irc_format!(
                                "🛸 Mother Ship Tractor Beam swapped {} and {}!",
                                ctx.nick(a).bold(),
                                ctx.nick(b).bold(),
                            ),
                        );
                    }
                    ctx.record(irc_format!(
                        "🛸 {} ({}) tractor-beamed a swap between {} ({}) and {} ({}).",
                        "Alien".bold().red(),
                        ctx.nick(actor).bold(),
                        ctx.nick(a).bold(),
                        card_a.display_for_peek(),
                        ctx.nick(b).bold(),
                        card_b.display_for_peek(),
                    ));
                    return;
                }
            }
            AlienDirective::CenterProbe { actor, slot, lone } => {
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
                if lone {
                    ctx.record(irc_format!(
                        "👽 Lone Alien {} peeked Center #{} ({}).",
                        ctx.nick(actor).bold().green(),
                        slot + 1,
                        seen.display_for_peek(),
                    ));
                } else {
                    ctx.record(irc_format!(
                        "🛸 Aliens ({}) probed Center #{} ({}).",
                        ctx.roster.format_seats(active_seats).bold(),
                        slot + 1,
                        seen.display_for_peek(),
                    ));
                }
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

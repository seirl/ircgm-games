use ircgm::prelude::*;

use super::oracle::OracleAlienExchange;
use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
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
    /// Standard ONUA Alien wake: `[Aliens]` coordination huddle, or Lone Alien 1-center peek.
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
            "Wake with the Aliens in a Mother Ship huddle and carry out the rolled Mother Ship event (recon, bio-scan, tractor beam, or center probe)!",
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
                        "🛸 Mother Ship Override (Oracle): Your Alien cards will exchange places at the end of this phase! Whisper here to coordinate.",
                    );
                }
                ctx.relay_huddle("Aliens".green().bold(), active_seats)
                    .await?;
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
            (AlienEvent::StandardRecon, None) => {
                ctx.relay_huddle("Aliens".green().bold(), active_seats)
                    .await?;
                AlienDirective::None
            }
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
        let preferred_actor = active_seats
            .into_iter()
            .find(|&s| ctx.waking_roles[s].id() == "alien")
            .or_else(|| {
                active_seats
                    .into_iter()
                    .find(|&s| ctx.waking_roles[s].id() != "synthetic_alien")
            })
            .unwrap_or_else(|| active_seats.first().expect("active_seats non-empty"));
        let non_synth: SeatSet = active_seats
            .into_iter()
            .filter(|&s| ctx.waking_roles[s].id() != "synthetic_alien")
            .collect();
        let authorized = if non_synth.is_empty() {
            active_seats
        } else {
            non_synth
        };

        let roster = ctx.roster;
        let huddle_cfg = AskConfig::new()
            .reply_deadline(ctx.deadline)
            .no_timeout_message();
        let mut huddle = ctx.huddle("Aliens".green().bold(), active_seats).await?;
        if huddle.is_empty() {
            return Ok(AlienDirective::None);
        }

        let is_solo = huddle.len() == 1;
        let non_aliens_str = ctx.format_visible_seats(non_alien_seats);
        let prompt = match event {
            AlienEvent::BioScan => irc_format!(
                "👽 Mother Ship Bio-Scan: {}Reply {} (or {}) to scan a non-Alien! Non-Aliens: {non_aliens_str}",
                if is_solo { "" } else { "Whisper here to coordinate, or " },
                if is_solo { "scan <nick>" } else { "!scan <nick>" }.bold(),
                if is_solo { "pass" } else { "!pass" }.bold(),
            ),
            AlienEvent::TractorBeam => irc_format!(
                "🛸 Mother Ship Tractor Beam: {}Reply {} (or {}) to swap two non-Aliens! Non-Aliens: {non_aliens_str}",
                if is_solo { "" } else { "Whisper here to coordinate, or " },
                if is_solo { "beam <nick1> <nick2>" } else { "!beam <nick1> <nick2>" }.bold(),
                if is_solo { "pass" } else { "!pass" }.bold(),
            ),
            AlienEvent::CenterProbe | AlienEvent::StandardRecon => irc_format!(
                "🛸 Mother Ship Center Probe: {}Reply {} (or {}) to probe 1 center card!",
                if is_solo { "" } else { "Whisper here to coordinate, or " },
                if is_solo { "1 / 2 / 3" } else { "!probe <1|2|3>" }.bold(),
                if is_solo { "pass" } else { "!pass" }.bold(),
            ),
        };

        let ans = huddle
            .ask(prompt, move |m| {
                let sender_seat = roster
                    .seats
                    .iter()
                    .position(|s| s.user.id() == m.sender.id())
                    .unwrap_or(preferred_actor);
                let raw = m.text.trim();
                let is_explicit_cmd = raw.starts_with('!');

                if !is_solo && !is_explicit_cmd {
                    return AskMatch::ignore();
                }
                if !authorized.contains(sender_seat) {
                    return AskMatch::reject(
                        "🤖 Only true Aliens can lock in the Mother Ship directive!".yellow(),
                    );
                }
                if Roster::is_pass(raw) {
                    return AskMatch::accept_say(
                        AlienDirective::None,
                        "👽 Passed — Mother Ship standing by.",
                    );
                }

                let stripped = raw.strip_prefix('!').unwrap_or(raw);
                let parts: Vec<&str> = stripped.split_whitespace().collect();
                match event {
                    AlienEvent::BioScan => {
                        let tok = if parts
                            .first()
                            .is_some_and(|w| w.eq_ignore_ascii_case("scan"))
                        {
                            if parts.len() != 2 {
                                return AskMatch::reject("Usage: !scan <nick>.".yellow());
                            }
                            parts[1]
                        } else if parts.len() == 1 {
                            parts[0]
                        } else {
                            return AskMatch::reject("Usage: !scan <nick> or !pass.".yellow());
                        };
                        match roster.resolve_nick(tok) {
                            Some(idx) if non_aliens.contains(idx) => AskMatch::accept_say(
                                AlienDirective::Scan {
                                    actor: sender_seat,
                                    target: idx,
                                },
                                irc_format!(
                                    "👽 Bio-Scan locked in on {} — result arrives when the phase ends!",
                                    roster.seats[idx].user.nick().bold(),
                                ),
                            ),
                            Some(idx) if shielded.contains(idx) => {
                                AskMatch::reject("🛡️ That player's card is shielded!".yellow())
                            }
                            Some(_) => AskMatch::reject("Pick a non-Alien player!".yellow()),
                            None => AskMatch::reject("Unknown player nickname.".yellow()),
                        }
                    }
                    AlienEvent::TractorBeam => {
                        let pair = if parts.first().is_some_and(|w| {
                            w.eq_ignore_ascii_case("beam") || w.eq_ignore_ascii_case("swap")
                        }) {
                            if parts.len() != 3 {
                                return AskMatch::reject(
                                    "Usage: !beam <nick1> <nick2>.".yellow(),
                                );
                            }
                            (parts[1], parts[2])
                        } else if parts.len() == 2 {
                            (parts[0], parts[1])
                        } else {
                            return AskMatch::reject(
                                "Usage: !beam <nick1> <nick2> or !pass.".yellow(),
                            );
                        };
                        let (Some(a), Some(b)) =
                            (roster.resolve_nick(pair.0), roster.resolve_nick(pair.1))
                        else {
                            return AskMatch::reject("Unknown player nickname.".yellow());
                        };
                        if a == b {
                            return AskMatch::reject("Pick two DIFFERENT players!".yellow());
                        }
                        if shielded.contains(a) || shielded.contains(b) {
                            return AskMatch::reject(
                                "🛡️ Shielded players cannot be tractor-beamed!".yellow(),
                            );
                        }
                        if !non_aliens.contains(a) || !non_aliens.contains(b) {
                            return AskMatch::reject("Pick two non-Alien players!".yellow());
                        }
                        AskMatch::accept_say(
                            AlienDirective::TractorBeam {
                                actor: sender_seat,
                                a,
                                b,
                            },
                            irc_format!(
                                "🛸 Tractor Beam locked in: swapping {} and {}!",
                                roster.seats[a].user.nick().bold(),
                                roster.seats[b].user.nick().bold(),
                            ),
                        )
                    }
                    AlienEvent::CenterProbe | AlienEvent::StandardRecon => {
                        let arg = if parts.first().is_some_and(|w| {
                            w.eq_ignore_ascii_case("probe") || w.eq_ignore_ascii_case("center")
                        }) {
                            if parts.len() != 2 {
                                return AskMatch::reject("Usage: !probe <1|2|3>.".yellow());
                            }
                            parts[1]
                        } else {
                            stripped
                        };
                        match Roster::parse_center_slot(arg) {
                            Some(slot) => AskMatch::accept_say(
                                AlienDirective::CenterProbe {
                                    actor: sender_seat,
                                    slot,
                                    lone: false,
                                },
                                format!(
                                    "🛸 Center Probe locked in on Center #{} — reveals when the phase ends!",
                                    slot + 1
                                ),
                            ),
                            None => AskMatch::reject("Usage: !probe <1|2|3> or !pass.".yellow()),
                        }
                    }
                }
            })
            .config(&huddle_cfg)
            .await?;

        if !is_solo {
            tokio::time::sleep_until(ctx.deadline).await;
        }
        drop(huddle);
        Ok(ans.ok().unwrap_or(AlienDirective::None))
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

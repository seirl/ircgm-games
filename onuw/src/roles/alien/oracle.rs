use ircgm::prelude::*;

use super::alien::ALIEN_PACK_STEP;
use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, SpecialVictory,
    Team, VictoryCtx, WakeStep,
};
use crate::roles::{
    base::werewolf::WOLF_PACK_STEP, super_villains::henchman::VILLAIN_PACK_STEP,
    vampire::vampire::VAMPIRE_PACK_STEP,
};

/// Set by `Oracle` Question #2 (`Alien Exchange = yes`) so `Aliens` exchange cards at `#1A`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct OracleAlienExchange(pub bool);

impl BoardKey for OracleAlienExchange {
    const ACCESS: u16 = BoardAccess::CUSTOM_0;

    fn init() -> Self {
        Self(false)
    }
}

/// Official *One Night Ultimate Alien* app question pool for `Oracle` (`#-9`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OracleQuestion {
    /// #1 (45%): Join an evil monster faction in the deck (`yes` / `no`).
    ChangeTeam(NightFaction),
    /// #2 (10%): Force Aliens to exchange cards (`yes`), or `no` -> nothing / Alien-team minion (`4.d`).
    AlienExchange { symp_on_no: bool },
    /// #3 (10%): Swap own card with a center card (`yes` / `no`), with app fizzle chance.
    DrunkExchange { fizzle: bool },
    /// #4 (10%): View 3 center cards (`yes` -> app reveals `count` in `1..=3`).
    CenterPeek { count: usize },
    /// #5 (10%): State whether Oracle's seat number (`1..=N`) is `even` or `odd` (announced publicly).
    EvenOdd,
    /// #6 (10%): Choose a player's card to view (possibly redirected; viewed target announced publicly).
    PlayerPeek { redirect: Option<usize> },
    /// #7 (5%): Guess `1..=10` (wrong -> global survival hunt; right -> all-night wake watch).
    NumberGuess { secret: u8 },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedOracleQuestion(pub Option<OracleQuestion>);

impl BoardKey for ForcedOracleQuestion {
    const ACCESS: u16 = BoardAccess::CUSTOM_0;

    fn init() -> Self {
        Self(None)
    }
}

struct OracleConverted {
    info: RoleInfo,
    steps: &'static [WakeStep],
    hunted: bool,
}

impl Role for OracleConverted {
    fn info(&self) -> RoleInfo {
        self.info
    }

    fn steps(&self) -> &'static [WakeStep] {
        self.steps
    }

    fn special_victory(&self, _seat: usize, _ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        self.hunted.then_some(SpecialVictory::Hunted)
    }
}

static ORACLE_WEREWOLF: OracleConverted = OracleConverted {
    info: RoleInfo::new(
        "oracle_werewolf",
        "Oracle [Werewolf]",
        GameSet::Alien,
        Team::Werewolf,
        "Oracle converted into a Werewolf.",
    )
    .night_faction(NightFaction::Werewolf),
    steps: &[WOLF_PACK_STEP],
    hunted: false,
};

static ORACLE_VAMPIRE: OracleConverted = OracleConverted {
    info: RoleInfo::new(
        "oracle_vampire",
        "Oracle [Vampire]",
        GameSet::Alien,
        Team::Vampire,
        "Oracle converted into a Vampire.",
    )
    .night_faction(NightFaction::Vampire),
    steps: &[VAMPIRE_PACK_STEP],
    hunted: false,
};

static ORACLE_ALIEN: OracleConverted = OracleConverted {
    info: RoleInfo::new(
        "oracle_alien",
        "Oracle [Alien]",
        GameSet::Alien,
        Team::Alien,
        "Oracle converted into an Alien.",
    )
    .night_faction(NightFaction::Alien),
    steps: &[ALIEN_PACK_STEP],
    hunted: false,
};

static ORACLE_VILLAIN: OracleConverted = OracleConverted {
    info: RoleInfo::new(
        "oracle_villain",
        "Oracle [Super Villain]",
        GameSet::Alien,
        Team::SuperVillain,
        "Oracle converted into a Super Villain.",
    )
    .night_faction(NightFaction::SuperVillain),
    steps: &[VILLAIN_PACK_STEP],
    hunted: false,
};

static ORACLE_ALIEN_TEAM: OracleConverted = OracleConverted {
    info: RoleInfo::new(
        "oracle_alien_team",
        "Oracle [Alien Team]",
        GameSet::Alien,
        Team::Alien,
        "Oracle on the Alien Team (not an Alien).",
    )
    .minion(),
    steps: &[],
    hunted: false,
};

static ORACLE_HUNTED: OracleConverted = OracleConverted {
    info: RoleInfo::new(
        "oracle_hunted",
        "Oracle [Hunted]",
        GameSet::Alien,
        Team::Independent,
        "Oracle wins iff surviving; everyone else wins iff Oracle dies.",
    ),
    steps: &[],
    hunted: true,
};

fn converted_faction_role(faction: NightFaction) -> &'static dyn Role {
    match faction {
        NightFaction::Werewolf => &ORACLE_WEREWOLF,
        NightFaction::Vampire => &ORACLE_VAMPIRE,
        NightFaction::Alien => &ORACLE_ALIEN,
        NightFaction::SuperVillain => &ORACLE_VILLAIN,
    }
}

pub struct Oracle;
pub static ORACLE: Oracle = Oracle;

const ORACLE_STEP: WakeStep =
    WakeStep::group(&ORACLE, 1, |seats, ctx| Box::pin(Oracle::wake(seats, ctx)))
        .run_when_unheld()
        .reads(BoardAccess::CUSTOM_0 | BoardAccess::SHIELDS)
        .writes(
            BoardAccess::PLAYER_FACES
                | BoardAccess::PLAYER_BECAME
                | BoardAccess::CENTER_FACES
                | BoardAccess::WAKING_ROLES
                | BoardAccess::CUSTOM_0,
        );

#[derive(Clone, Copy)]
enum OracleAnswer {
    YesNo(bool),
    DrunkChoice(Option<usize>),
    Parity { is_even: bool },
    PlayerTarget(usize),
    Guess(u8),
}

impl Role for Oracle {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "oracle",
            "Oracle",
            GameSet::Alien,
            Team::Village,
            "Wake first at the start of Dusk/Night to answer a randomized Mother Ship question that can alter teams, cards, or victory rules!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ORACLE_STEP] }
    }
}

impl Oracle {
    fn roll_question(roster: &Roster) -> OracleQuestion {
        let candidate_factions: Vec<NightFaction> = [
            NightFaction::Werewolf,
            NightFaction::Vampire,
            NightFaction::Alien,
            NightFaction::SuperVillain,
        ]
        .into_iter()
        .filter(|&fac| {
            roster
                .deck
                .iter()
                .any(|r| r.steps().iter().any(|st| st.pack_faction() == Some(fac)))
        })
        .collect();

        let alien_pack_cards = roster
            .deck
            .iter()
            .filter(|r| r.night_faction() == Some(NightFaction::Alien))
            .count();

        let mut pool: Vec<(u32, OracleQuestion)> = Vec::with_capacity(7);
        if !candidate_factions.is_empty() {
            let fac = candidate_factions[fastrand::usize(..candidate_factions.len())];
            pool.push((45, OracleQuestion::ChangeTeam(fac)));
        }
        if alien_pack_cards >= 2 {
            pool.push((
                10,
                OracleQuestion::AlienExchange {
                    symp_on_no: fastrand::bool(),
                },
            ));
        }
        pool.push((
            10,
            OracleQuestion::DrunkExchange {
                fizzle: fastrand::u8(..4) == 0,
            },
        ));
        let center_count = match fastrand::u8(..20) {
            0..=8 => 1,
            9..=16 => 2,
            _ => 3,
        };
        pool.push((10, OracleQuestion::CenterPeek { count: center_count }));
        pool.push((10, OracleQuestion::EvenOdd));
        let redirect = (fastrand::u8(..10) < 3).then(|| fastrand::usize(..roster.len()));
        pool.push((10, OracleQuestion::PlayerPeek { redirect }));
        pool.push((
            5,
            OracleQuestion::NumberGuess {
                secret: fastrand::u8(1..=10),
            },
        ));

        let total: u32 = pool.iter().map(|(w, _)| *w).sum();
        let mut pick = fastrand::u32(..total);
        for (w, q) in pool {
            if pick < w {
                return q;
            }
            pick -= w;
        }
        OracleQuestion::EvenOdd
    }

    fn parse_yes_no(raw: &str) -> Option<bool> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "yes" | "y" => Some(true),
            "no" | "n" | "pass" | "skip" => Some(false),
            _ => None,
        }
    }

    async fn wake(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let question = ctx
            .get_key::<ForcedOracleQuestion>()
            .0
            .unwrap_or_else(|| Self::roll_question(ctx.roster));

        let actor_opt = active_seats.first();
        for extra in active_seats.into_iter().skip(1) {
            ctx.dm(
                extra,
                "🔮🛸 Oracle: Another Oracle is answering the Mother Ship's question tonight!",
            );
        }

        let answer_opt: Option<OracleAnswer> = if let Some(seat) = actor_opt {
            let roster = ctx.roster;
            let shielded = ctx.shielded_seats();
            match question {
                OracleQuestion::ChangeTeam(fac) => {
                    let prompt = irc_format!(
                        "🔮🛸 Oracle: Do you want to join the {} team and wake with them tonight? Reply {} or {}!",
                        fac.team().styled_label(),
                        "yes".bold(),
                        "no".bold(),
                    );
                    ctx.ask(seat, prompt, |m| match Self::parse_yes_no(&m.text) {
                        Some(v) => AskMatch::accept_say(
                            OracleAnswer::YesNo(v),
                            "🔮🛸 Answer recorded — resolves when the phase ends!",
                        ),
                        None => AskMatch::reject("Reply yes or no.".yellow()),
                    })
                    .await?
                    .ok()
                }
                OracleQuestion::AlienExchange { .. } => {
                    let prompt = irc_format!(
                        "🔮🛸 Oracle: Do you want to force the Aliens to exchange cards tonight? Reply {} or {}!",
                        "yes".bold(),
                        "no".bold(),
                    );
                    ctx.ask(seat, prompt, |m| match Self::parse_yes_no(&m.text) {
                        Some(v) => AskMatch::accept_say(
                            OracleAnswer::YesNo(v),
                            "🔮🛸 Answer recorded — resolves when the phase ends!",
                        ),
                        None => AskMatch::reject("Reply yes or no.".yellow()),
                    })
                    .await?
                    .ok()
                }
                OracleQuestion::DrunkExchange { .. } => {
                    let prompt = irc_format!(
                        "🔮🛸 Oracle: Do you want to exchange your card with a center card? Reply {} (or {}), or {}!",
                        "1 / 2 / 3".bold(),
                        "yes".bold(),
                        "no".bold(),
                    );
                    ctx.ask(seat, prompt, |m| {
                        let raw = m.text.trim();
                        if let Some(c_idx) = Roster::parse_center_slot(raw) {
                            return AskMatch::accept_say(
                                OracleAnswer::DrunkChoice(Some(c_idx)),
                                format!(
                                    "🔮🛸 Locked in exchanging with Center #{} — resolves when the phase ends!",
                                    c_idx + 1
                                ),
                            );
                        }
                        match Self::parse_yes_no(raw) {
                            Some(true) => AskMatch::accept_say(
                                OracleAnswer::DrunkChoice(Some(fastrand::usize(0..3))),
                                "🔮🛸 Locked in center exchange — resolves when the phase ends!",
                            ),
                            Some(false) => AskMatch::accept_say(
                                OracleAnswer::DrunkChoice(None),
                                "🔮🛸 Declined center exchange.",
                            ),
                            None => AskMatch::reject("Reply 1, 2, 3, yes, or no.".yellow()),
                        }
                    })
                    .await?
                    .ok()
                }
                OracleQuestion::CenterPeek { .. } => {
                    let prompt = irc_format!(
                        "🔮🛸 Oracle: Do you want to view the center cards? Reply {} or {}!",
                        "yes".bold(),
                        "no".bold(),
                    );
                    ctx.ask(seat, prompt, |m| match Self::parse_yes_no(&m.text) {
                        Some(v) => AskMatch::accept_say(
                            OracleAnswer::YesNo(v),
                            "🔮🛸 Answer recorded — vision arrives when the phase ends!",
                        ),
                        None => AskMatch::reject("Reply yes or no.".yellow()),
                    })
                    .await?
                    .ok()
                }
                OracleQuestion::EvenOdd => {
                    let circle = roster.format_table_circle();
                    let prompt = irc_format!(
                        "🔮🛸 Oracle: Your seat is #{} in {} — Reply {} or {} for your player number (announced publicly)!",
                        seat + 1,
                        circle,
                        "even".bold(),
                        "odd".bold(),
                    );
                    ctx.ask(seat, prompt, |m| {
                        match m.text.trim().to_ascii_lowercase().as_str() {
                            "even" | "e" => AskMatch::accept_say(
                                OracleAnswer::Parity { is_even: true },
                                "🔮🛸 Locked in EVEN!",
                            ),
                            "odd" | "o" => AskMatch::accept_say(
                                OracleAnswer::Parity { is_even: false },
                                "🔮🛸 Locked in ODD!",
                            ),
                            _ => AskMatch::reject("Reply even or odd.".yellow()),
                        }
                    })
                    .await?
                    .ok()
                }
                OracleQuestion::PlayerPeek { .. } => {
                    let others = roster.all_seats().without(seat) - shielded;
                    if others.is_empty() {
                        ctx.dm(
                            seat,
                            "🛡️ Oracle: Every other player's card is shielded!",
                        );
                        None
                    } else {
                        let players_str = ctx.format_visible_seats(roster.all_seats().without(seat));
                        let prompt = irc_format!(
                            "🔮🛸 Oracle: Reply {} to view another player's card (viewed player is announced publicly)! Players: {players_str}",
                            "<nick>".bold(),
                        );
                        ctx.ask(seat, prompt, move |m| match roster.resolve_nick(&m.text) {
                            Some(idx) if idx == seat => {
                                AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                            }
                            Some(idx) if shielded.contains(idx) => {
                                AskMatch::reject("🛡️ That player's card is shielded!".yellow())
                            }
                            Some(idx) => AskMatch::accept_say(
                                OracleAnswer::PlayerTarget(idx),
                                irc_format!(
                                    "🔮🛸 Locked in viewing {} — resolves when the phase ends!",
                                    roster.seats[idx].user.nick().bold(),
                                ),
                            ),
                            None => AskMatch::reject("Unknown player nickname.".yellow()),
                        })
                        .await?
                        .ok()
                    }
                }
                OracleQuestion::NumberGuess { .. } => {
                    let prompt = irc_format!(
                        "🔮🛸 Oracle: Reply a number from {} to {} to guess the Mother Ship's secret number!",
                        "1".bold(),
                        "10".bold(),
                    );
                    ctx.ask(seat, prompt, |m| match m.text.trim().parse::<u8>() {
                        Ok(n @ 1..=10) => AskMatch::accept_say(
                            OracleAnswer::Guess(n),
                            format!("🔮🛸 Locked in number {n}!"),
                        ),
                        _ => AskMatch::reject("Reply a number from 1 to 10.".yellow()),
                    })
                    .await?
                    .ok()
                }
            }
        } else {
            None
        };

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(active_seats, question, answer_opt, board, rctx);
        }))
    }

    fn resolve(
        active_seats: SeatSet,
        question: OracleQuestion,
        answer_opt: Option<OracleAnswer>,
        board: &mut BoardMut<'_>,
        rctx: &mut ResolveCtx<'_>,
    ) {
        let actor_opt = active_seats.first();
        match question {
            OracleQuestion::ChangeTeam(fac) => {
                let yes = match answer_opt {
                    Some(OracleAnswer::YesNo(v)) => v,
                    _ => fastrand::bool(),
                };
                if yes {
                    let new_role = converted_faction_role(fac);
                    board.convert_role_publicly(
                        active_seats,
                        ORACLE.info().id,
                        new_role,
                        true,
                        rctx,
                    );
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {} card has become a member of the {} team!",
                        "Oracle Omen:".bold().cyan(),
                        ORACLE.info().styled_name(),
                        fac.team().styled_label(),
                    ));
                    for s in active_seats {
                        rctx.dm(
                            s,
                            irc_format!(
                                "🔮🛸 You are now a full {} — stand by to wake with them!",
                                fac.team().styled_label(),
                            ),
                        );
                    }
                    rctx.record(irc_format!(
                        "🔮🛸 Oracle card joined the {} team ({}).",
                        fac.team().styled_label(),
                        if active_seats.is_empty() {
                            "in center".to_owned()
                        } else {
                            rctx.roster.format_seats(active_seats)
                        },
                    ));
                } else {
                    rctx.record("🔮🛸 Oracle declined to change teams (stayed Village).");
                }
            }

            OracleQuestion::AlienExchange { symp_on_no } => {
                let yes = match answer_opt {
                    Some(OracleAnswer::YesNo(v)) => v,
                    _ => fastrand::bool(),
                };
                if yes {
                    board.set_key(OracleAlienExchange(true));
                    if let Some(s) = actor_opt {
                        rctx.dm(
                            s,
                            "🔮🛸 The Aliens will be forced to exchange cards when they wake!",
                        );
                    }
                    rctx.record("🔮🛸 Oracle forced the Aliens to exchange cards tonight.");
                } else if symp_on_no {
                    board.convert_role_publicly(
                        active_seats,
                        ORACLE.info().id,
                        &ORACLE_ALIEN_TEAM,
                        false,
                        rctx,
                    );
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {} card is now on the {} Team, but is NOT an Alien!",
                        "Oracle Omen:".bold().cyan(),
                        ORACLE.info().styled_name(),
                        Team::Alien.styled_label(),
                    ));
                    for s in active_seats {
                        rctx.dm(
                            s,
                            "🔮🛸 You are now on the Alien Team (minion — you win with the Aliens, and dying does not kill an Alien)!",
                        );
                    }
                    rctx.record("🔮🛸 Oracle card joined the Alien Team as a non-Alien sympathizer.");
                } else {
                    rctx.record("🔮🛸 Oracle declined Alien Exchange (remained Village).");
                }
            }

            OracleQuestion::DrunkExchange { fizzle } => {
                let Some(seat) = actor_opt else {
                    rctx.record("🔮🛸 Oracle was in the center (center exchange skipped).");
                    return;
                };
                let slot_opt = match answer_opt {
                    Some(OracleAnswer::DrunkChoice(opt)) => opt,
                    _ => fastrand::bool().then(|| fastrand::usize(0..3)),
                };
                match slot_opt {
                    Some(_) if fizzle => {
                        rctx.dm(
                            seat,
                            "🔮🛸 Mother Ship static aborted the exchange — your card stays where it is!",
                        );
                        rctx.record(irc_format!(
                            "🔮🛸 Oracle ({}) attempted a center swap, but the Mother Ship fizzled it.",
                            rctx.nick(seat).bold(),
                        ));
                    }
                    Some(slot) => {
                        if let Some((_, center_card)) =
                            board.swap_player_with_center(seat, seat, slot)
                        {
                            rctx.dm(
                                seat,
                                format!(
                                    "🔮🛸 Exchanged your card with Center #{} (without looking)!",
                                    slot + 1
                                ),
                            );
                            rctx.record(irc_format!(
                                "🔮🛸 Oracle ({}) exchanged cards with Center #{} ({}).",
                                rctx.nick(seat).bold(),
                                slot + 1,
                                center_card.display_for_peek(),
                            ));
                        }
                    }
                    None => {
                        rctx.record(irc_format!(
                            "🔮🛸 Oracle ({}) declined to exchange with the center.",
                            rctx.nick(seat).bold(),
                        ));
                    }
                }
            }

            OracleQuestion::CenterPeek { count } => {
                let Some(seat) = actor_opt else {
                    rctx.record("🔮🛸 Oracle was in the center (center peek skipped).");
                    return;
                };
                let yes = match answer_opt {
                    Some(OracleAnswer::YesNo(v)) => v,
                    _ => true,
                };
                if !yes {
                    rctx.record(irc_format!(
                        "🔮🛸 Oracle ({}) declined to view center cards.",
                        rctx.nick(seat).bold(),
                    ));
                    return;
                }
                let mut slots = [0usize, 1, 2];
                fastrand::shuffle(&mut slots);
                let chosen = &slots[..count.clamp(1, 3)];
                let mut parts = Vec::new();
                for &c_idx in chosen {
                    let seen = board.peek_center(seat, c_idx);
                    parts.push(irc_format!("#{} {}", c_idx + 1, seen.display_for_peek()));
                }
                let joined = Text::join(&parts, ", ".grey());
                rctx.dm(
                    seat,
                    irc_format!(
                        "🔮🛸 Mother Ship granted {}/3 center card(s): {}",
                        chosen.len(),
                        joined.clone(),
                    ),
                );
                rctx.record(irc_format!(
                    "🔮🛸 Oracle ({}) viewed Center {}.",
                    rctx.nick(seat).bold(),
                    joined,
                ));
            }

            OracleQuestion::EvenOdd => {
                let is_even = match answer_opt {
                    Some(OracleAnswer::Parity { is_even }) => is_even,
                    _ => match actor_opt {
                        Some(s) => (s + 1) % 2 == 0,
                        None => fastrand::bool(),
                    },
                };
                let word = if is_even { "EVEN" } else { "ODD" };
                rctx.announce(irc_format!(
                    "🔮🛸 {} The Oracle states their player seat number (1..={}) is {}!",
                    "Oracle Omen:".bold().cyan(),
                    rctx.roster.len(),
                    word.bold().yellow(),
                ));
                rctx.record(format!("🔮🛸 Oracle publicly stated their seat number is {word}."));
            }

            OracleQuestion::PlayerPeek { redirect } => {
                if let Some(seat) = actor_opt {
                    let candidates = board.unshielded_other_seats(seat);
                    if let Some(fallback) = candidates.choose_random() {
                        let requested = match answer_opt {
                            Some(OracleAnswer::PlayerTarget(idx)) if candidates.contains(idx) => idx,
                            _ => fallback,
                        };
                        let target = redirect
                            .filter(|&r| r != seat && candidates.contains(r))
                            .unwrap_or(requested);
                        if let Some(seen) = board.peek_player(seat, target) {
                            let target_nick = rctx.nick(target);
                            rctx.dm(
                                seat,
                                irc_format!(
                                    "🔮🛸 Oracle Vision: {} holds {}!",
                                    target_nick.as_str().bold(),
                                    seen.display_for_peek(),
                                ),
                            );
                            rctx.announce(irc_format!(
                                "🔮🛸 {} {}'s card was viewed by the Oracle!",
                                "Oracle Omen:".bold().cyan(),
                                target_nick.as_str().bold().yellow(),
                            ));
                            rctx.record(irc_format!(
                                "🔮🛸 Oracle ({}) viewed {}'s card ({}).",
                                rctx.nick(seat).bold(),
                                target_nick.bold(),
                                seen.display_for_peek(),
                            ));
                            return;
                        }
                    }
                } else {
                    let decoys = rctx.roster.all_seats();
                    if let Some(target) = redirect
                        .filter(|&r| decoys.contains(r))
                        .or_else(|| decoys.choose_random())
                    {
                        let target_nick = rctx.nick(target);
                        rctx.announce(irc_format!(
                            "🔮🛸 {} {}'s card was viewed by the Oracle!",
                            "Oracle Omen:".bold().cyan(),
                            target_nick.as_str().bold().yellow(),
                        ));
                        rctx.record(format!(
                            "🔮🛸 Oracle (in center) announced decoy peek on {target_nick}."
                        ));
                    }
                }
            }

            OracleQuestion::NumberGuess { secret } => {
                let guess = match answer_opt {
                    Some(OracleAnswer::Guess(n)) => n,
                    _ => fastrand::u8(1..=10),
                };
                if guess == secret {
                    if let Some(seat) = actor_opt {
                        rctx.grant_wake_log(seat);
                        rctx.dm(
                            seat,
                            format!(
                                "🔮🛸 JACKPOT ({secret})! You guessed the Mother Ship's exact number — you stay awake all night and will receive the full wake log before Dawn!"
                            ),
                        );
                        rctx.record(irc_format!(
                            "🔮🛸 Oracle ({}) guessed secret number {secret} and stayed wide awake!",
                            rctx.nick(seat).bold(),
                        ));
                    } else {
                        rctx.record(format!(
                            "🔮🛸 Center Oracle matched secret number {secret} (silent)."
                        ));
                    }
                } else {
                    board.convert_role_publicly(
                        active_seats,
                        ORACLE.info().id,
                        &ORACLE_HUNTED,
                        false,
                        rctx,
                    );
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The Oracle guessed wrong! The Oracle ONLY wins if the Oracle survives — ALL OTHER PLAYERS ONLY win if the Oracle is killed!",
                        "Oracle Omen:".bold().red(),
                    ));
                    for s in active_seats {
                        rctx.dm(
                            s,
                            format!(
                                "🔮🛸 Wrong guess ({guess} vs {secret})! Survive the vote to win solo — everyone else wins ONLY if you die!"
                            ),
                        );
                    }
                    rctx.record(format!(
                        "🔮🛸 Oracle missed secret number ({guess} vs {secret}) — global Oracle Hunt activated!"
                    ));
                }
            }
        }
    }
}

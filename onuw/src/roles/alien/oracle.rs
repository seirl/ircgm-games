use futures::future::try_join_all;
use ircgm::prelude::*;

use super::alien::ALIEN_PACK_STEP;
use crate::board::{BoardAccess, BoardKey, BoardMut, CenterSlot, Roster, SeatSet};
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
    const ACCESS: u16 = BoardAccess::ROLE_STATE;

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
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedDoppelgangerOracleQuestion(pub Option<OracleQuestion>);

impl BoardKey for ForcedDoppelgangerOracleQuestion {
    const ACCESS: u16 = BoardAccess::NONE;

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

const ORACLE_WRITES: u16 = BoardAccess::PLAYER_FACES
    | BoardAccess::PLAYER_BECAME
    | BoardAccess::CENTER_FACES
    | BoardAccess::WAKING_ROLES
    | BoardAccess::ROLE_STATE
    | BoardAccess::PUBLIC_ANNOUNCE;

const ORACLE_STEP: WakeStep =
    WakeStep::group(&ORACLE, |seats, ctx| Box::pin(Oracle::wake(seats, ctx)))
        .run_when_unheld()
        .copy_after_player_copier()
        .reads(BoardAccess::SHIELDS)
        .writes(ORACLE_WRITES);

#[derive(Clone, Copy)]
enum OracleAnswer {
    YesNo(bool),
    DrunkChoice(Option<CenterSlot>),
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
                fizzle: fastrand::u8(..10) < 3,
            },
        ));
        pool.push((
            10,
            OracleQuestion::CenterPeek {
                count: fastrand::usize(1..=3),
            },
        ));
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

    async fn prompt_one(
        seat: usize,
        title: &'static str,
        question: OracleQuestion,
        ctx: &GroupPromptCtx<'_>,
    ) -> Result<Option<OracleAnswer>> {
        let roster = ctx.roster;
        let shielded = ctx.shielded_seats();
        match question {
            OracleQuestion::ChangeTeam(fac) => {
                let prompt = irc_format!(
                    "🔮🛸 {title}: Do you want to join the {} team and wake with them tonight? Reply {} or {}!",
                    fac.team().styled_label(),
                    "yes".bold(),
                    "no".bold(),
                );
                Ok(ctx
                    .ask(seat, prompt, |m| match Self::parse_yes_no(&m.text) {
                        Some(v) => AskMatch::accept_say(
                            OracleAnswer::YesNo(v),
                            "🔮🛸 Answer recorded — resolves when the phase ends!",
                        ),
                        None => AskMatch::reject("Reply yes or no.".yellow()),
                    })
                    .await?
                    .ok())
            }
            OracleQuestion::AlienExchange { .. } => {
                let prompt = irc_format!(
                    "🔮🛸 {title}: Do you want to force the Aliens to exchange cards tonight? Reply {} or {}!",
                    "yes".bold(),
                    "no".bold(),
                );
                Ok(ctx
                    .ask(seat, prompt, |m| match Self::parse_yes_no(&m.text) {
                        Some(v) => AskMatch::accept_say(
                            OracleAnswer::YesNo(v),
                            "🔮🛸 Answer recorded — resolves when the phase ends!",
                        ),
                        None => AskMatch::reject("Reply yes or no.".yellow()),
                    })
                    .await?
                    .ok())
            }
            OracleQuestion::DrunkExchange { .. } => {
                let prompt = irc_format!(
                    "🔮🛸 {title}: Do you want to exchange your card with a center card? Reply {} (or {}), or {}!",
                    roster.center_slash_hint().bold(),
                    "yes".bold(),
                    "no".bold(),
                );
                Ok(ctx
                    .ask(seat, prompt, move |m| {
                        let raw = m.text.trim();
                        if let Some(slot) = roster.parse_center_slot(raw) {
                            return AskMatch::accept_say(
                                OracleAnswer::DrunkChoice(Some(slot)),
                                format!(
                                    "🔮🛸 Locked in exchanging with {} — resolves when the phase ends!",
                                    slot.label(roster)
                                ),
                            );
                        }
                        match Self::parse_yes_no(raw) {
                            Some(true) => AskMatch::accept_say(
                                OracleAnswer::DrunkChoice(Some(roster.choose_random_center_slot())),
                                "🔮🛸 Locked in center exchange — resolves when the phase ends!",
                            ),
                            Some(false) => AskMatch::accept_say(
                                OracleAnswer::DrunkChoice(None),
                                "🔮🛸 Declined center exchange.",
                            ),
                            None => AskMatch::reject(
                                format!("Reply {}, yes, or no.", roster.center_choice_hint())
                                    .yellow(),
                            ),
                        }
                    })
                    .await?
                    .ok())
            }
            OracleQuestion::CenterPeek { .. } => {
                let prompt = irc_format!(
                    "🔮🛸 {title}: Do you want to view the center cards? Reply {} or {}!",
                    "yes".bold(),
                    "no".bold(),
                );
                Ok(ctx
                    .ask(seat, prompt, |m| match Self::parse_yes_no(&m.text) {
                        Some(v) => AskMatch::accept_say(
                            OracleAnswer::YesNo(v),
                            "🔮🛸 Answer recorded — vision arrives when the phase ends!",
                        ),
                        None => AskMatch::reject("Reply yes or no.".yellow()),
                    })
                    .await?
                    .ok())
            }
            OracleQuestion::EvenOdd => {
                let circle = roster.format_table_circle();
                let prompt = irc_format!(
                    "🔮🛸 {title}: Your seat is #{} in {} — Reply {} or {} for your player number (announced publicly)!",
                    seat + 1,
                    circle,
                    "even".bold(),
                    "odd".bold(),
                );
                Ok(ctx
                    .ask(seat, prompt, |m| {
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
                    .ok())
            }
            OracleQuestion::PlayerPeek { .. } => {
                let others = roster.all_seats().without(seat) - shielded;
                if others.is_empty() {
                    ctx.dm(
                        seat,
                        format!("🛡️ {title}: Every other player's card is shielded!"),
                    );
                    Ok(None)
                } else {
                    let players_str = roster.format_seats(roster.all_seats().without(seat));
                    let prompt = irc_format!(
                        "🔮🛸 {title}: Reply {} to view another player's card (viewed player is announced publicly)! Players: {players_str}",
                        "<nick>".bold(),
                    );
                    Ok(ctx
                        .ask(seat, prompt, move |m| match roster.resolve_nick(&m.text) {
                            Some(idx) if idx == seat => {
                                AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                            }
                            Some(idx) if shielded.contains(idx) => {
                                AskMatch::reject("🛡️ That player's card is shielded!".yellow())
                            }
                            Some(idx) => AskMatch::accept_say(
                                OracleAnswer::PlayerTarget(idx),
                                format!(
                                    "🔮🛸 Locked in {} — vision arrives when the phase ends!",
                                    roster.seats[idx].user.nick()
                                ),
                            ),
                            None => AskMatch::reject("Unknown player nickname.".yellow()),
                        })
                        .await?
                        .ok())
                }
            }
            OracleQuestion::NumberGuess { .. } => {
                let prompt = irc_format!(
                    "🔮🛸 {title}: Guess a number from {} (Reply {})! Wrong = survive-or-everyone-hunts-you; Right = keep your eyes open all night!",
                    "1 to 10".bold(),
                    "1..=10".bold(),
                );
                Ok(ctx
                    .ask(seat, prompt, |m| {
                        match m.text.trim().parse::<u8>() {
                            Ok(n @ 1..=10) => AskMatch::accept_say(
                                OracleAnswer::Guess(n),
                                format!(
                                    "🔮🛸 Guessed {n} — fate reveals when the phase ends!"
                                ),
                            ),
                            _ => AskMatch::reject(
                                "Reply with a number between 1 and 10.".yellow(),
                            ),
                        }
                    })
                    .await?
                    .ok())
            }
        }
    }

    async fn wake(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        if ctx.is_player_copy_turn() {
            return Self::wake_doppelganger(active_seats, ctx).await;
        }

        let forced = ctx.get_key::<ForcedOracleQuestion>().0;
        let total_oracle_cards = ctx
            .roster
            .deck
            .iter()
            .filter(|r| r.id() == "oracle")
            .count()
            .max(1);
        let unheld_count = total_oracle_cards.saturating_sub(active_seats.len());

        let ctx_ref = &ctx;
        let held_outcomes = try_join_all(active_seats.into_iter().map(|seat| async move {
            let q = forced.unwrap_or_else(|| Self::roll_question(ctx_ref.roster));
            let ans = Self::prompt_one(seat, "Oracle", q, ctx_ref).await?;
            Ok::<_, Error>((seat, q, ans))
        }))
        .await?;

        let center_questions: Vec<OracleQuestion> = (0..unheld_count)
            .map(|_| forced.unwrap_or_else(|| Self::roll_question(ctx.roster)))
            .collect();

        Ok(GroupEffect::new(move |board, rctx| {
            for (seat, q, ans) in held_outcomes {
                Self::resolve_one(Some(seat), false, q, ans, board, rctx);
            }
            for q in center_questions {
                Self::resolve_one(None, false, q, None, board, rctx);
            }
        }))
    }

    async fn wake_doppelganger(
        active_seats: SeatSet,
        ctx: GroupPromptCtx<'_>,
    ) -> Result<GroupEffect> {
        let forced = ctx.get_key::<ForcedDoppelgangerOracleQuestion>().0;
        let total_player_copiers = ctx
            .roster
            .deck
            .iter()
            .filter(|r| r.is_player_copier())
            .count()
            .max(1);
        let decoy_count = total_player_copiers.saturating_sub(active_seats.len());

        let ctx_ref = &ctx;
        let held_outcomes = try_join_all(active_seats.into_iter().map(|seat| async move {
            let q = forced.unwrap_or_else(|| Self::roll_question(ctx_ref.roster));
            let ans = Self::prompt_one(seat, "Doppelgänger-Oracle", q, ctx_ref).await?;
            Ok::<_, Error>((seat, q, ans))
        }))
        .await?;

        Ok(GroupEffect::new(move |board, rctx| {
            for (seat, q, ans) in held_outcomes {
                Self::resolve_one(Some(seat), true, q, ans, board, rctx);
            }
            for _ in 0..decoy_count {
                Self::resolve_doppelganger_decoy(rctx);
            }
        }))
    }

    /// Emits a non-mutating decoy omen when `Doppelgänger` is in the deck alongside `Oracle`
    /// but did not copy `Oracle` (or `Doppelgänger` is in the center), preventing copy-target leaks.
    fn resolve_doppelganger_decoy(rctx: &mut ResolveCtx<'_>) {
        let banner = "Doppelgänger-Oracle Omen:".bold().cyan();
        let subject = Team::Village.style_text("Doppelgänger-Oracle");
        match fastrand::u8(..4) {
            0 => {
                rctx.announce(irc_format!(
                    "🔮🛸 {banner} Asked to exchange with a center card — the {subject} declined!"
                ));
            }
            1 => {
                let n = fastrand::usize(1..=3);
                rctx.announce(irc_format!(
                    "🔮🛸 {banner} The {subject} was granted a vision of {n} center card(s)!"
                ));
            }
            2 => {
                let word = if fastrand::bool() { "EVEN" } else { "ODD" };
                rctx.announce(irc_format!(
                    "🔮🛸 {banner} The {subject} states their player seat number (1..={}) is {}!",
                    rctx.roster.len(),
                    word.bold().yellow(),
                ));
            }
            _ => {
                if let Some(target) = rctx.roster.all_seats().choose_random() {
                    let target_nick = rctx.nick(target);
                    rctx.announce(irc_format!(
                        "🔮🛸 {banner} {}'s card was viewed by the {subject}!",
                        target_nick.as_str().bold().yellow(),
                    ));
                }
            }
        }
        rctx.record("🔮🛸 Doppelgänger-Oracle emitted a decoy omen (Doppelgänger did not copy Oracle).");
    }

    fn resolve_one(
        actor_opt: Option<usize>,
        is_doppelganger: bool,
        question: OracleQuestion,
        answer_opt: Option<OracleAnswer>,
        board: &mut BoardMut<'_>,
        rctx: &mut ResolveCtx<'_>,
    ) {
        let target_seats = match actor_opt {
            Some(s) => SeatSet::single(s),
            None => SeatSet::EMPTY,
        };
        let omen_tag = if is_doppelganger {
            "Doppelgänger-Oracle Omen:"
        } else {
            "Oracle Omen:"
        };
        let role_title = if is_doppelganger {
            "Doppelgänger-Oracle"
        } else {
            "Oracle"
        };
        let styled_subject = if is_doppelganger {
            Team::Village.style_text("Doppelgänger-Oracle")
        } else {
            ORACLE.info().styled_name()
        };

        match question {
            OracleQuestion::ChangeTeam(fac) => {
                let yes = match answer_opt {
                    Some(OracleAnswer::YesNo(v)) => v,
                    _ => fastrand::bool(),
                };
                if yes {
                    let new_role = converted_faction_role(fac);
                    board.convert_role_publicly(
                        target_seats,
                        ORACLE.info().id,
                        new_role,
                        true,
                        rctx,
                    );
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {} card has become a member of the {} team!",
                        omen_tag.bold().cyan(),
                        styled_subject,
                        fac.team().styled_label(),
                    ));
                    for s in target_seats {
                        rctx.dm(
                            s,
                            irc_format!(
                                "🔮🛸 You are now a full {} — stand by to wake with them!",
                                fac.team().styled_label(),
                            ),
                        );
                    }
                    rctx.record(irc_format!(
                        "🔮🛸 {role_title} card joined the {} team ({}).",
                        fac.team().styled_label(),
                        if target_seats.is_empty() {
                            "in center".to_owned()
                        } else {
                            rctx.roster.format_seats(target_seats)
                        },
                    ));
                } else {
                    rctx.announce(irc_format!(
                        "🔮🛸 {} Asked to join the {} team — the {} declined and remains Village!",
                        omen_tag.bold().cyan(),
                        fac.team().styled_label(),
                        styled_subject,
                    ));
                    rctx.record(format!(
                        "🔮🛸 {role_title} declined to change teams (stayed Village)."
                    ));
                }
            }

            OracleQuestion::AlienExchange { symp_on_no } => {
                let yes = match answer_opt {
                    Some(OracleAnswer::YesNo(v)) => v,
                    _ => fastrand::bool(),
                };
                if yes {
                    board.set_key(OracleAlienExchange(true));
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {} ordered the {} to exchange cards tonight!",
                        omen_tag.bold().cyan(),
                        styled_subject,
                        Team::Alien.style_text("Aliens"),
                    ));
                    if let Some(s) = actor_opt {
                        rctx.dm(
                            s,
                            "🔮🛸 The Aliens will be forced to exchange cards when they wake!",
                        );
                    }
                    rctx.record(format!(
                        "🔮🛸 {role_title} forced the Aliens to exchange cards tonight."
                    ));
                } else if symp_on_no {
                    board.convert_role_publicly(
                        target_seats,
                        ORACLE.info().id,
                        &ORACLE_ALIEN_TEAM,
                        false,
                        rctx,
                    );
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {} card is now on the {} Team, but is NOT an Alien!",
                        omen_tag.bold().cyan(),
                        styled_subject,
                        Team::Alien.styled_label(),
                    ));
                    for s in target_seats {
                        rctx.dm(
                            s,
                            "🔮🛸 You are now on the Alien Team (minion — you win with the Aliens, and dying does not kill an Alien)!",
                        );
                    }
                    rctx.record(format!(
                        "🔮🛸 {role_title} card joined the Alien Team as a non-Alien sympathizer."
                    ));
                } else {
                    rctx.announce(irc_format!(
                        "🔮🛸 {} Asked to force an Alien card exchange — the {} declined!",
                        omen_tag.bold().cyan(),
                        styled_subject,
                    ));
                    rctx.record(format!(
                        "🔮🛸 {role_title} declined Alien Exchange (remained Village)."
                    ));
                }
            }

            OracleQuestion::DrunkExchange { fizzle } => {
                let slot_opt = match answer_opt {
                    Some(OracleAnswer::DrunkChoice(opt)) => opt,
                    _ => fastrand::bool().then(|| rctx.roster.choose_random_center_slot()),
                };
                match slot_opt {
                    Some(_) if fizzle => {
                        rctx.announce(irc_format!(
                            "🔮🛸 {} The {} chose to exchange with a center card, but Mother Ship static aborted the exchange!",
                            omen_tag.bold().cyan(),
                            styled_subject,
                        ));
                        if let Some(seat) = actor_opt {
                            rctx.dm(
                                seat,
                                "🔮🛸 Mother Ship static aborted the exchange — your card stays where it is!",
                            );
                            rctx.record(irc_format!(
                                "🔮🛸 {role_title} ({}) attempted a center swap, but the Mother Ship fizzled it.",
                                rctx.nick(seat).bold(),
                            ));
                        } else {
                            rctx.record(format!("🔮🛸 Center {role_title} decoy center swap fizzled."));
                        }
                    }
                    Some(slot) => {
                        rctx.announce(irc_format!(
                            "🔮🛸 {} The {} chose to exchange their card with a center card!",
                            omen_tag.bold().cyan(),
                            styled_subject,
                        ));
                        if let Some(seat) = actor_opt {
                            if let Some((_, center_card)) =
                                board.swap_player_with_center(seat, seat, slot)
                            {
                                let slot_label = slot.label(rctx.roster);
                                rctx.dm(
                                    seat,
                                    format!(
                                        "🔮🛸 Exchanged your card with {slot_label} (without looking)!"
                                    ),
                                );
                                rctx.record(irc_format!(
                                    "🔮🛸 {role_title} ({}) exchanged cards with {slot_label} ({}).",
                                    rctx.nick(seat).bold(),
                                    center_card.display_for_peek(),
                                ));
                            }
                        } else {
                            rctx.record(format!(
                                "🔮🛸 {role_title} was in the center (announced decoy center swap)."
                            ));
                        }
                    }
                    None => {
                        rctx.announce(irc_format!(
                            "🔮🛸 {} Asked to exchange with a center card — the {} declined!",
                            omen_tag.bold().cyan(),
                            styled_subject,
                        ));
                        if let Some(seat) = actor_opt {
                            rctx.record(irc_format!(
                                "🔮🛸 {role_title} ({}) declined to exchange with the center.",
                                rctx.nick(seat).bold(),
                            ));
                        } else {
                            rctx.record(format!(
                                "🔮🛸 {role_title} was in the center (announced decoy decline)."
                            ));
                        }
                    }
                }
            }

            OracleQuestion::CenterPeek { count } => {
                let yes = match answer_opt {
                    Some(OracleAnswer::YesNo(v)) => v,
                    _ => actor_opt.is_some() || fastrand::bool(),
                };
                let n_cards = count.clamp(1, 3);
                if !yes {
                    rctx.announce(irc_format!(
                        "🔮🛸 {} Asked to view center cards — the {} declined!",
                        omen_tag.bold().cyan(),
                        styled_subject,
                    ));
                    if let Some(seat) = actor_opt {
                        rctx.record(irc_format!(
                            "🔮🛸 {role_title} ({}) declined to view center cards.",
                            rctx.nick(seat).bold(),
                        ));
                    } else {
                        rctx.record(format!(
                            "🔮🛸 {role_title} was in the center (announced decoy center-peek decline)."
                        ));
                    }
                    return;
                }
                rctx.announce(irc_format!(
                    "🔮🛸 {} The {} was granted a vision of {n_cards} center card(s)!",
                    omen_tag.bold().cyan(),
                    styled_subject,
                ));
                let Some(seat) = actor_opt else {
                    rctx.record(format!(
                        "🔮🛸 {role_title} was in the center (announced decoy {n_cards}-card center peek)."
                    ));
                    return;
                };
                let mut slots = rctx.roster.center_slots();
                let total_center = slots.len();
                fastrand::shuffle(&mut slots);
                let chosen = &slots[..n_cards.min(total_center)];
                let mut parts = Vec::new();
                for &slot in chosen {
                    let seen = board.peek_center(seat, slot);
                    parts.push(irc_format!(
                        "{} {}",
                        slot.short_label(rctx.roster),
                        seen.display_for_peek()
                    ));
                }
                let joined = Text::join(&parts, ", ".grey());
                rctx.dm(
                    seat,
                    irc_format!(
                        "🔮🛸 Mother Ship granted {}/{total_center} center card(s): {}",
                        chosen.len(),
                        joined.clone(),
                    ),
                );
                rctx.record(irc_format!(
                    "🔮🛸 {role_title} ({}) viewed Center {}.",
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
                    "🔮🛸 {} The {role_title} states their player seat number (1..={}) is {}!",
                    omen_tag.bold().cyan(),
                    rctx.roster.len(),
                    word.bold().yellow(),
                ));
                rctx.record(format!(
                    "🔮🛸 {role_title} publicly stated their seat number is {word}."
                ));
            }

            OracleQuestion::PlayerPeek { redirect } => {
                if let Some(seat) = actor_opt {
                    let candidates = board.unshielded_other_seats(seat);
                    if let Some(fallback) = candidates.choose_random() {
                        let requested = match answer_opt {
                            Some(OracleAnswer::PlayerTarget(idx)) if candidates.contains(idx) => {
                                idx
                            }
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
                                    "🔮🛸 {role_title} Vision: {} holds {}!",
                                    target_nick.as_str().bold(),
                                    seen.display_for_peek(),
                                ),
                            );
                            rctx.announce(irc_format!(
                                "🔮🛸 {} {}'s card was viewed by the {role_title}!",
                                omen_tag.bold().cyan(),
                                target_nick.as_str().bold().yellow(),
                            ));
                            rctx.record(irc_format!(
                                "🔮🛸 {role_title} ({}) viewed {}'s card ({}).",
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
                            "🔮🛸 {} {}'s card was viewed by the {role_title}!",
                            omen_tag.bold().cyan(),
                            target_nick.as_str().bold().yellow(),
                        ));
                        rctx.record(format!(
                            "🔮🛸 {role_title} (in center) announced decoy peek on {target_nick}."
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
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {} guessed the Mother Ship's secret number ({secret}) and keeps their eyes OPEN all night!",
                        omen_tag.bold().cyan(),
                        styled_subject,
                    ));
                    if let Some(seat) = actor_opt {
                        rctx.grant_wake_log(seat);
                        rctx.dm(
                            seat,
                            format!(
                                "🔮🛸 JACKPOT ({secret})! You guessed the Mother Ship's exact number — you stay awake all night and will receive the full wake log before Dawn!"
                            ),
                        );
                        rctx.record(irc_format!(
                            "🔮🛸 {role_title} ({}) guessed secret number {secret} and stayed wide awake!",
                            rctx.nick(seat).bold(),
                        ));
                    } else {
                        rctx.record(format!(
                            "🔮🛸 Center {role_title} matched secret number {secret} (announced decoy watch)."
                        ));
                    }
                } else {
                    board.convert_role_publicly(
                        target_seats,
                        ORACLE.info().id,
                        &ORACLE_HUNTED,
                        false,
                        rctx,
                    );
                    rctx.announce(irc_format!(
                        "🔮🛸 {} The {role_title} guessed wrong! The {role_title} ONLY wins if they survive — ALL OTHER PLAYERS ONLY win if the {role_title} is killed!",
                        omen_tag.bold().red(),
                    ));
                    for s in target_seats {
                        rctx.dm(
                            s,
                            format!(
                                "🔮🛸 Wrong guess ({guess} vs {secret})! Survive the vote to win solo — everyone else wins ONLY if you die!"
                            ),
                        );
                    }
                    rctx.record(format!(
                        "🔮🛸 {role_title} missed secret number ({guess} vs {secret}) — global Oracle Hunt activated!"
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "oracle_test.rs"]
mod test;

//! Official **Secret Hitler** rules, 17-tile [`PolicyDeck`], [`Roster`], pure [`BoardState`],
//! input parsers, and backtick-free IRC formatters.

use std::collections::VecDeque;

use ircgm::prelude::*;

pub const MIN_PLAYERS: usize = 5;
pub const MAX_PLAYERS: usize = 10;
pub const LIBERAL_POLICIES_TO_WIN: usize = 5;
pub const FASCIST_POLICIES_TO_WIN: usize = 6;
pub const HITLER_TERRITORY_THRESHOLD: usize = 3;
pub const VETO_UNLOCK_THRESHOLD: usize = 5;
pub const CHAOS_ELECTION_THRESHOLD: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Party {
    Liberal,
    Fascist,
}

impl Party {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Liberal => "Liberal",
            Self::Fascist => "Fascist",
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Self::Liberal => "LIBERAL".bold().cyan(),
            Self::Fascist => "FASCIST".bold().red(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Liberal,
    Fascist,
    Hitler,
}

impl Role {
    /// Rule (Investigate Loyalty): Hitler's Party Membership card reads Fascist.
    pub const fn party(self) -> Party {
        match self {
            Self::Liberal => Party::Liberal,
            Self::Fascist | Self::Hitler => Party::Fascist,
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Self::Liberal => "Liberal".bold().cyan(),
            Self::Fascist => "Fascist".bold().red(),
            Self::Hitler => "Hitler".bold().orange(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Policy {
    Liberal,
    Fascist,
}

impl Policy {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Liberal => "Liberal",
            Self::Fascist => "Fascist",
        }
    }

    pub fn label(self) -> Text {
        match self {
            Self::Liberal => "LIBERAL".bold().cyan(),
            Self::Fascist => "FASCIST".bold().red(),
        }
    }

    pub fn short(self) -> Text {
        match self {
            Self::Liberal => "L".bold().cyan(),
            Self::Fascist => "F".bold().red(),
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input
            .trim()
            .trim_start_matches('!')
            .to_ascii_lowercase()
            .as_str()
        {
            "liberal" | "lib" | "l" | "blue" => Some(Self::Liberal),
            "fascist" | "fas" | "f" | "red" => Some(Self::Fascist),
            _ => None,
        }
    }
}

pub fn format_policies(cards: &[Policy]) -> Text {
    Text::join(cards.iter().copied().map(Policy::label), ", ".grey())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChancellorChoice {
    Enact(Policy),
    ProposeVeto,
}

/// Parses President's 1-of-3 **discard** choice, rejecting inverted `enact`/`pass`/`keep`.
pub fn parse_president_discard(raw: &str) -> std::result::Result<Policy, &'static str> {
    let words: Vec<&str> = raw
        .trim()
        .trim_start_matches('!')
        .split_whitespace()
        .collect();
    match words.as_slice() {
        [p] => Policy::parse(p),
        [v, p] if matches!(v.to_ascii_lowercase().as_str(), "discard" | "drop") => Policy::parse(p),
        [v, ..]
            if matches!(
                v.to_ascii_lowercase().as_str(),
                "enact" | "pass" | "play" | "keep"
            ) =>
        {
            return Err("As President, choose the 1 policy to DISCARD (liberal / fascist)!");
        }
        _ => None,
    }
    .ok_or("Choose 1 policy to DISCARD: reply liberal (l) or fascist (f).")
}

/// Parses Chancellor's 1-of-2 **enact** (or `veto`) choice, rejecting inverted `discard`/`drop`.
pub fn parse_chancellor_choice(
    raw: &str,
    can_veto: bool,
) -> std::result::Result<ChancellorChoice, &'static str> {
    let words: Vec<&str> = raw
        .trim()
        .trim_start_matches('!')
        .split_whitespace()
        .collect();
    let usage = if can_veto {
        "Choose 1 policy to ENACT (liberal / fascist), or reply veto."
    } else {
        "Choose 1 policy to ENACT: reply liberal (l) or fascist (f)."
    };
    match words.as_slice() {
        [w] if w.eq_ignore_ascii_case("veto") => {
            can_veto.then_some(ChancellorChoice::ProposeVeto).ok_or(
                "Veto Power is not available (unlocks at 5 Fascist Policies, once per session).",
            )
        }
        [p] => Policy::parse(p).map(ChancellorChoice::Enact).ok_or(usage),
        [v, p] if matches!(v.to_ascii_lowercase().as_str(), "enact" | "play" | "pass") => {
            Policy::parse(p).map(ChancellorChoice::Enact).ok_or(usage)
        }
        [v, ..] if matches!(v.to_ascii_lowercase().as_str(), "discard" | "drop") => {
            Err("As Chancellor, choose the policy to ENACT onto the board (liberal / fascist)!")
        }
        _ => Err(usage),
    }
}

/// Parses a `Ja!` (`true`) or `Nein!` (`false`) ballot or veto consent reply.
pub fn parse_vote(raw: &str) -> Option<bool> {
    let words: Vec<&str> = raw
        .trim()
        .trim_start_matches('!')
        .split_whitespace()
        .collect();
    let tok = match words.as_slice() {
        [w, arg] if w.eq_ignore_ascii_case("vote") => *arg,
        [only] => *only,
        _ => return None,
    };
    match tok.trim_end_matches('!').to_ascii_lowercase().as_str() {
        "ja" | "yes" | "y" | "oui" => Some(true),
        "nein" | "no" | "n" | "non" => Some(false),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExecPower {
    Peek,
    Investigate,
    SpecialElection,
    Execution,
}

impl ExecPower {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Peek => "Policy Peek",
            Self::Investigate => "Investigate Loyalty",
            Self::SpecialElection => "Call Special Election",
            Self::Execution => "Execution",
        }
    }

    pub const fn tag(self) -> &'static str {
        match self {
            Self::Peek => "Peek",
            Self::Investigate => "Inv",
            Self::SpecialElection => "Elect",
            Self::Execution => "Kill",
        }
    }
}

/// Official `(liberals, regular_fascists)` counts alongside 1 `Hitler` for 5–10 players.
pub const fn role_distribution(n: usize) -> Option<(usize, usize)> {
    match n {
        5 => Some((3, 1)),
        6 => Some((4, 1)),
        7 => Some((4, 2)),
        8 => Some((5, 2)),
        9 => Some((5, 3)),
        10 => Some((6, 3)),
        _ => None,
    }
}

/// Rule (Setup): Hitler knows the Fascist in 5–6p games, and does NOT know them in 7–10p games.
pub const fn hitler_knows_fascists(n: usize) -> bool {
    n <= 6
}

/// Powers on Fascist track slots 1..=5 for a `5..=10` player game.
pub const fn fascist_slot_powers(total_players: usize) -> Option<[Option<ExecPower>; 5]> {
    use ExecPower::*;
    match total_players {
        5..=6 => Some([None, None, Some(Peek), Some(Execution), Some(Execution)]),
        7..=8 => Some([
            None,
            Some(Investigate),
            Some(SpecialElection),
            Some(Execution),
            Some(Execution),
        ]),
        9..=10 => Some([
            Some(Investigate),
            Some(Investigate),
            Some(SpecialElection),
            Some(Execution),
            Some(Execution),
        ]),
        _ => None,
    }
}

pub const fn exec_power(total_players: usize, fas_policies: usize) -> Option<ExecPower> {
    match (fascist_slot_powers(total_players), fas_policies) {
        (Some(slots), 1..=5) => slots[fas_policies - 1],
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WinReason {
    FiveLiberalPolicies,
    SixFascistPolicies,
    HitlerElectedChancellor { chancellor: usize },
    HitlerExecuted { president: usize, hitler: usize },
}

impl WinReason {
    pub const fn winning_party(self) -> Party {
        match self {
            Self::FiveLiberalPolicies | Self::HitlerExecuted { .. } => Party::Liberal,
            Self::SixFascistPolicies | Self::HitlerElectedChancellor { .. } => Party::Fascist,
        }
    }
}

/// 17-tile Policy deck (6 Liberal + 11 Fascist). Front (`draw[0]`) is the top tile.
#[derive(Clone, Debug)]
pub struct PolicyDeck {
    draw: VecDeque<Policy>,
    discard: Vec<Policy>,
}

impl PolicyDeck {
    pub fn new_shuffled() -> Self {
        let mut cards = vec![Policy::Liberal; 6];
        cards.extend(std::iter::repeat_n(Policy::Fascist, 11));
        fastrand::shuffle(&mut cards);
        Self::from_draw_order(cards)
    }

    pub fn from_draw_order<I: IntoIterator<Item = Policy>>(cards: I) -> Self {
        Self {
            draw: cards.into_iter().collect(),
            discard: Vec::new(),
        }
    }

    pub fn draw_len(&self) -> usize {
        self.draw.len()
    }

    pub fn discard_len(&self) -> usize {
        self.discard.len()
    }

    fn ensure(&mut self, needed: usize) {
        if self.draw.len() < needed && !self.discard.is_empty() {
            let mut all: Vec<Policy> = self.draw.drain(..).collect();
            all.append(&mut self.discard);
            fastrand::shuffle(&mut all);
            self.draw = VecDeque::from(all);
        }
    }

    pub fn draw_one(&mut self) -> Policy {
        self.ensure(1);
        self.draw.pop_front().expect("policy deck non-empty")
    }

    pub fn draw_three(&mut self) -> [Policy; 3] {
        self.ensure(3);
        [self.draw_one(), self.draw_one(), self.draw_one()]
    }

    pub fn peek_top_three(&mut self) -> [Policy; 3] {
        self.ensure(3);
        [self.draw[0], self.draw[1], self.draw[2]]
    }

    pub fn discard(&mut self, cards: impl IntoIterator<Item = Policy>) {
        self.discard.extend(cards);
    }

    /// Rule (Legislative Session & Election Tracker): reshuffle remaining draw + discard whenever
    /// fewer than 3 tiles remain at the end of a Legislative Session or Chaos enactment.
    pub fn reshuffle_if_needed(&mut self) -> Option<usize> {
        (self.draw.len() < 3 && !self.discard.is_empty()).then(|| {
            self.ensure(3);
            self.draw.len()
        })
    }
}

#[derive(Clone, Debug)]
pub struct SeatInfo {
    pub user: User,
    pub role: Role,
}

#[derive(Clone, Debug)]
pub struct Roster {
    pub seats: Vec<SeatInfo>,
}

impl Roster {
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }

    pub fn nick(&self, seat: usize) -> String {
        self.seats[seat].user.nick()
    }

    pub fn seat_of(&self, user: &User) -> Option<usize> {
        self.seats.iter().position(|s| s.user.id() == user.id())
    }

    pub fn resolve_nick(&self, raw: &str) -> Option<usize> {
        let trimmed = raw.trim().trim_end_matches([':', ',', '.', '!']).trim();
        (!trimmed.is_empty())
            .then(|| self.seats.iter().position(|s| s.user.eq_nick(trimmed)))
            .flatten()
    }

    pub fn hitler_seat(&self) -> usize {
        self.seats
            .iter()
            .position(|s| s.role == Role::Hitler)
            .expect("one Hitler")
    }

    pub fn format_seats(&self, seats: &[usize]) -> Text {
        let items: Vec<Text> = seats.iter().map(|&i| self.nick(i).bold().into()).collect();
        Text::join(&items, ", ".grey())
    }

    pub fn format_table_order(&self, board: &BoardState) -> Text {
        let items: Vec<Text> = (0..self.len())
            .map(|i| {
                let nick = self.nick(i);
                if !board.is_alive(i) {
                    irc_format!("{} {}", nick.as_str().grey(), "💀".grey())
                } else if board.is_investigated(i) {
                    irc_format!("{} {}", nick.as_str().bold(), "[🔍]".cyan())
                } else {
                    nick.as_str().bold().into()
                }
            })
            .collect();
        irc_format!("{} {}", Text::join(&items, " → ".grey()), "(wraps)".grey())
    }
}

/// Pure rules state machine for an active game of **Secret Hitler**.
#[derive(Clone, Debug)]
pub struct BoardState {
    pub total_players: usize,
    alive: Vec<bool>,
    investigated: Vec<bool>,
    pub deck: PolicyDeck,
    pub lib_policies: usize,
    pub fas_policies: usize,
    pub election_tracker: usize,
    pub round: usize,
    pub current_president: usize,
    next_regular_president: usize,
    pub special_president: Option<usize>,
    pub is_special_election: bool,
    pub last_elected_president: Option<usize>,
    pub last_elected_chancellor: Option<usize>,
}

impl BoardState {
    pub fn new(total_players: usize, initial_president: usize, deck: PolicyDeck) -> Self {
        assert!(initial_president < total_players);
        Self {
            total_players,
            alive: vec![true; total_players],
            investigated: vec![false; total_players],
            deck,
            lib_policies: 0,
            fas_policies: 0,
            election_tracker: 0,
            round: 0,
            current_president: initial_president,
            next_regular_president: initial_president,
            special_president: None,
            is_special_election: false,
            last_elected_president: None,
            last_elected_chancellor: None,
        }
    }

    pub fn is_alive(&self, seat: usize) -> bool {
        self.alive.get(seat).copied().unwrap_or(false)
    }

    pub fn is_investigated(&self, seat: usize) -> bool {
        self.investigated.get(seat).copied().unwrap_or(false)
    }

    pub fn alive_count(&self) -> usize {
        self.alive.iter().filter(|&&a| a).count()
    }

    pub fn alive_seats(&self) -> Vec<usize> {
        (0..self.total_players).filter(|&s| self.alive[s]).collect()
    }

    pub fn mark_investigated(&mut self, target: usize) {
        self.investigated[target] = true;
    }

    /// Rule (Call Special Election): After a Special Election, the Presidency returns to the left
    /// of the President who called it (so the next player in line can run twice in a row).
    pub fn advance_president(&mut self) -> usize {
        self.round += 1;
        if let Some(sp) = self.special_president.take().filter(|&s| self.alive[s]) {
            self.current_president = sp;
            self.is_special_election = true;
            return sp;
        }
        self.is_special_election = false;
        while !self.alive[self.next_regular_president] {
            self.next_regular_president = (self.next_regular_president + 1) % self.total_players;
        }
        self.current_president = self.next_regular_president;
        self.next_regular_president = (self.current_president + 1) % self.total_players;
        self.current_president
    }

    /// Rule (Eligibility): Last elected Chancellor (and last elected President while `> 5` living
    /// players remain) are term-limited; failed nominations never alter term limits.
    pub fn chancellor_ineligible_reason(&self, cand: usize) -> Option<&'static str> {
        if !self.is_alive(cand) {
            Some("That player has been executed!")
        } else if cand == self.current_president {
            Some("You cannot nominate yourself as Chancellor!")
        } else if self.last_elected_chancellor == Some(cand) {
            Some("That player is term-limited (last elected Chancellor)!")
        } else if self.alive_count() > 5 && self.last_elected_president == Some(cand) {
            Some("That player is term-limited (last elected President)!")
        } else {
            None
        }
    }

    pub fn eligible_chancellors(&self) -> Vec<usize> {
        (0..self.total_players)
            .filter(|&s| self.chancellor_ineligible_reason(s).is_none())
            .collect()
    }

    pub fn term_limited_seats(&self) -> Vec<usize> {
        (0..self.total_players)
            .filter(|&s| {
                self.alive[s]
                    && s != self.current_president
                    && self.chancellor_ineligible_reason(s).is_some()
            })
            .collect()
    }

    /// Rule (Investigate Loyalty): No player may be investigated twice in the same game.
    pub fn eligible_investigation_targets(&self) -> Vec<usize> {
        (0..self.total_players)
            .filter(|&s| self.alive[s] && s != self.current_president && !self.investigated[s])
            .collect()
    }

    pub fn eligible_other_living(&self) -> Vec<usize> {
        (0..self.total_players)
            .filter(|&s| self.alive[s] && s != self.current_president)
            .collect()
    }

    pub fn record_failed_election(&mut self) -> bool {
        self.election_tracker += 1;
        self.election_tracker >= CHAOS_ELECTION_THRESHOLD
    }

    /// Records a passed election without resetting `election_tracker` yet (so a consented Veto
    /// during the Legislative Session still advances `election_tracker` toward Chaos).
    pub fn record_elected_government(
        &mut self,
        chancellor: usize,
        role: Role,
    ) -> Option<WinReason> {
        if self.fas_policies >= HITLER_TERRITORY_THRESHOLD && role == Role::Hitler {
            return Some(WinReason::HitlerElectedChancellor { chancellor });
        }
        self.last_elected_president = Some(self.current_president);
        self.last_elected_chancellor = Some(chancellor);
        None
    }

    /// Rule (Veto Power): Each consented Veto advances the Election Tracker by 1 (toward Chaos).
    pub fn record_consented_veto(&mut self, remaining_two: [Policy; 2]) -> bool {
        self.deck.discard(remaining_two);
        self.election_tracker += 1;
        self.election_tracker >= CHAOS_ELECTION_THRESHOLD
    }

    pub fn enact_chaos_policy(&mut self) -> (Policy, Option<WinReason>) {
        self.election_tracker = 0;
        self.last_elected_president = None;
        self.last_elected_chancellor = None;
        let top = self.deck.draw_one();
        let (win, _) = self.apply_enacted_policy(top, true);
        (top, win)
    }

    pub fn apply_enacted_policy(
        &mut self,
        policy: Policy,
        from_chaos: bool,
    ) -> (Option<WinReason>, Option<ExecPower>) {
        self.election_tracker = 0;
        match policy {
            Policy::Liberal => {
                self.lib_policies += 1;
                let win = (self.lib_policies >= LIBERAL_POLICIES_TO_WIN)
                    .then_some(WinReason::FiveLiberalPolicies);
                (win, None)
            }
            Policy::Fascist => {
                self.fas_policies += 1;
                if self.fas_policies >= FASCIST_POLICIES_TO_WIN {
                    (Some(WinReason::SixFascistPolicies), None)
                } else {
                    let power = (!from_chaos)
                        .then(|| exec_power(self.total_players, self.fas_policies))
                        .flatten();
                    (None, power)
                }
            }
        }
    }

    pub fn execute_player(&mut self, victim: usize, victim_role: Role) -> Option<WinReason> {
        self.alive[victim] = false;
        (victim_role == Role::Hitler).then_some(WinReason::HitlerExecuted {
            president: self.current_president,
            hitler: victim,
        })
    }

    pub fn format_board_banner(&self, roster: &Roster) -> Text {
        let lib = Text::join(
            (1..=LIBERAL_POLICIES_TO_WIN).map(|i| match i {
                i if i <= self.lib_policies => "[L]".bold().cyan(),
                LIBERAL_POLICIES_TO_WIN => "[🕊️]".cyan(),
                _ => "[·]".grey(),
            }),
            "",
        );
        let powers = fascist_slot_powers(self.total_players).unwrap_or([None; 5]);
        let fas = Text::join(
            (1..=FASCIST_POLICIES_TO_WIN).map(|i| {
                match (i, powers.get(i - 1).copied().flatten()) {
                    (i, _) if i <= self.fas_policies => "[F]".bold().red(),
                    (FASCIST_POLICIES_TO_WIN, _) => "[💀]".red(),
                    (5, Some(p)) => format!("[{}+Veto]", p.tag()).orange(),
                    (i, Some(p)) if i >= HITLER_TERRITORY_THRESHOLD => {
                        format!("[{}]", p.tag()).orange()
                    }
                    (_, Some(p)) => format!("[{}]", p.tag()).grey(),
                    _ => "[·]".grey(),
                }
            }),
            "",
        );
        let dots = match self.election_tracker {
            0 => "○○○".grey(),
            1 => "●○○".yellow(),
            2 => "●●○".bold().orange(),
            _ => "●●●".bold().red(),
        };
        let sep = " · ".grey();
        let mut flags = Text::default();
        if self.fas_policies >= HITLER_TERRITORY_THRESHOLD {
            flags = flags + sep.clone() + "⚠️ Hitler Zone".bold().orange();
        }
        if self.fas_policies >= VETO_UNLOCK_THRESHOLD {
            flags = flags + sep.clone() + "🛑 Veto Active".bold().yellow();
        }
        let pres_lbl = if self.is_special_election {
            "🎖️ Special Pres:"
        } else {
            "🎩 Pres:"
        };
        irc_format!(
            "🏛️ {} {lib} {fas}{sep}Tracker: {}/3 {dots}{sep}{}{sep}{pres_lbl} {}{flags}",
            format!("Round {}", self.round).bold(),
            self.election_tracker,
            format!(
                "Deck: {} ({} discard)",
                self.deck.draw_len(),
                self.deck.discard_len()
            )
            .grey(),
            roster.nick(self.current_president).bold().yellow(),
        )
    }

    pub fn format_topic(&self, roster: &Roster, phase: &str) -> String {
        let hz = if self.fas_policies >= HITLER_TERRITORY_THRESHOLD {
            " ⚠️HitlerZone"
        } else {
            ""
        };
        format!(
            "Secret Hitler — R{} · Lib {}/5 · Fas {}/6{hz} · Tracker {}/3 · Pres: {} ({phase})",
            self.round,
            self.lib_policies,
            self.fas_policies,
            self.election_tracker,
            roster.nick(self.current_president),
        )
    }
}

pub fn format_track_reference(label: &str, sample_players: usize) -> Text {
    let powers = fascist_slot_powers(sample_players).expect("valid bracket");
    let mut slots: Vec<Text> = powers
        .into_iter()
        .enumerate()
        .map(|(idx, p_opt)| match (idx + 1, p_opt) {
            (3, Some(p)) => format!("#3 {} + ⚠️ Hitler Zone", p.name()).bold().into(),
            (5, Some(p)) => format!("#5 {} + 🛑 Veto", p.name()).bold().into(),
            (n, Some(p)) => format!("#{n} {}", p.name()).bold().into(),
            (n, None) => format!("#{n} —").grey(),
        })
        .collect();
    slots.push("#6 💀 Fascist Win".bold().red());
    irc_format!(
        "  {} {}",
        format!("{label}:").bold().cyan(),
        Text::join(&slots, " · ".grey())
    )
}

#[cfg(test)]
#[path = "state_test.rs"]
mod test;

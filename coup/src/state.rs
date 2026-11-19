//! Core **Coup** + **Inquisitor** & **Reformation** domain types, official deck scaling,
//! pure [`BoardState`], targeting/Allegiance rules, and deck swap mutations.

use ircgm::prelude::*;
use ircgm::CaseMap;

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 10;

pub const COUP_COST: u32 = 7;
pub const MANDATORY_COUP_COINS: u32 = 10;
pub const ASSASSINATE_COST: u32 = 3;
pub const CONVERT_SELF_COST: u32 = 1;
pub const CONVERT_OTHER_COST: u32 = 2;

/// Official copies per character role: 3x for 2–6p, 4x for 7–8p, 5x for 9–10p.
pub const fn copies_per_role(player_count: usize) -> Option<usize> {
    match player_count {
        2..=6 => Some(3),
        7..=8 => Some(4),
        9..=10 => Some(5),
        _ => None,
    }
}

/// Variant flags selectable via `!start [inquisition|reformation]` or host `!mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameMode {
    pub inquisitor: bool,
    pub reformation: bool,
}

impl GameMode {
    pub const fn classic() -> Self {
        Self {
            inquisitor: false,
            reformation: false,
        }
    }
    pub const fn inquisitor() -> Self {
        Self {
            inquisitor: true,
            reformation: false,
        }
    }
    pub const fn reformation() -> Self {
        Self {
            inquisitor: false,
            reformation: true,
        }
    }
    pub const fn reformation_inquisitor() -> Self {
        Self {
            inquisitor: true,
            reformation: true,
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Some(Self::classic());
        }
        let (mut mode, mut saw_amb, mut saw_tok) = (Self::classic(), false, false);
        for tok in trimmed
            .split(|c: char| c.is_whitespace() || matches!(c, '+' | ',' | '/'))
            .filter(|s| !s.is_empty())
        {
            saw_tok = true;
            match tok.to_ascii_lowercase().as_str() {
                "classic" | "base" | "standard" | "normal" => {}
                "ambassador" => saw_amb = true,
                "inquisitor" | "inquisition" | "inq" => mode.inquisitor = true,
                "reformation" | "ref" | "factions" | "faction" | "allegiances" | "allegiance" => {
                    mode.reformation = true;
                }
                "full" | "all" | "both" => {
                    mode.inquisitor = true;
                    mode.reformation = true;
                }
                _ => return None,
            }
        }
        (saw_tok && !(saw_amb && mode.inquisitor)).then_some(mode)
    }

    pub const fn active_roles(self) -> [Role; 5] {
        let swap = if self.inquisitor {
            Role::Inquisitor
        } else {
            Role::Ambassador
        };
        [
            Role::Duke,
            Role::Assassin,
            Role::Captain,
            swap,
            Role::Contessa,
        ]
    }

    pub const fn exchange_draw_count(self) -> usize {
        if self.inquisitor {
            1
        } else {
            2
        }
    }

    pub const fn label(self) -> &'static str {
        match (self.reformation, self.inquisitor) {
            (false, false) => "Classic Coup",
            (false, true) => "Coup: Inquisitor",
            (true, false) => "Coup: Reformation",
            (true, true) => "Coup: Reformation + Inquisitor",
        }
    }

    pub fn styled_label(self) -> Text {
        let b = self.label().bold();
        match (self.reformation, self.inquisitor) {
            (false, false) => b.cyan(),
            (false, true) => b.yellow(),
            (true, false) => b.orange(),
            (true, true) => b.magenta(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Duke,
    Assassin,
    Captain,
    Ambassador,
    Inquisitor,
    Contessa,
}

impl Role {
    pub const ALL: [Role; 6] = [
        Role::Duke,
        Role::Assassin,
        Role::Captain,
        Role::Ambassador,
        Role::Inquisitor,
        Role::Contessa,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Role::Duke => "Duke",
            Role::Assassin => "Assassin",
            Role::Captain => "Captain",
            Role::Ambassador => "Ambassador",
            Role::Inquisitor => "Inquisitor",
            Role::Contessa => "Contessa",
        }
    }

    pub fn styled(self) -> Text {
        let b = self.name().bold();
        match self {
            Role::Duke => b.magenta(),
            Role::Assassin => b.orange(),
            Role::Captain => b.cyan(),
            Role::Ambassador => b.green(),
            Role::Inquisitor => b.yellow(),
            Role::Contessa => b.red(),
        }
    }

    pub const fn summary(self) -> &'static str {
        match self {
            Role::Duke => "Tax (+3 coins); blocks Foreign Aid; cannot Embezzle in Reformation.",
            Role::Assassin => {
                "Pay 3 coins to Assassinate a target (lose 1 influence); blocked by Contessa."
            }
            Role::Captain => "Steal up to 2 coins from a target; blocks Steal.",
            Role::Ambassador => {
                "Exchange: draw 2 cards from Court Deck, keep your live count, return 2; blocks Steal."
            }
            Role::Inquisitor => {
                "Exchange 1 card with Court Deck, or Examine a target's chosen card and keep/force-swap it; blocks Steal."
            }
            Role::Contessa => "Blocks Assassinate.",
        }
    }
}

pub fn parse_role(raw: &str) -> Option<Role> {
    match raw
        .trim()
        .trim_start_matches('!')
        .to_ascii_lowercase()
        .as_str()
    {
        "duke" | "d" => Some(Role::Duke),
        "assassin" | "ass" | "assn" => Some(Role::Assassin),
        "captain" | "cap" | "capt" => Some(Role::Captain),
        "ambassador" | "amb" => Some(Role::Ambassador),
        "inquisitor" | "inquisition" | "inq" | "i" => Some(Role::Inquisitor),
        "contessa" | "cont" | "con" => Some(Role::Contessa),
        _ => None,
    }
}

pub fn format_numbered_roles(roles: &[Role]) -> Text {
    let items: Vec<Text> = roles
        .iter()
        .enumerate()
        .map(|(i, r)| irc_format!("{}: {}", i + 1, r.styled()))
        .collect();
    Text::join(&items, " · ".grey())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Allegiance {
    Loyalist,
    Reformist,
}

impl Allegiance {
    pub const fn opposite(self) -> Self {
        match self {
            Self::Loyalist => Self::Reformist,
            Self::Reformist => Self::Loyalist,
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Self::Loyalist => "Loyalist".bold().cyan(),
            Self::Reformist => "Reformist".bold().orange(),
        }
    }

    pub fn short_styled(self) -> Text {
        match self {
            Self::Loyalist => "L".bold().cyan(),
            Self::Reformist => "R".bold().orange(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardSlot {
    pub role: Role,
    pub revealed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerState {
    pub cards: [CardSlot; 2],
    pub coins: u32,
    pub allegiance: Option<Allegiance>,
}

impl PlayerState {
    pub fn new(cards: [Role; 2], coins: u32, allegiance: Option<Allegiance>) -> Self {
        Self {
            cards: cards.map(|role| CardSlot {
                role,
                revealed: false,
            }),
            coins,
            allegiance,
        }
    }

    pub fn live_slots(&self) -> Vec<(usize, Role)> {
        self.cards
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.revealed)
            .map(|(i, c)| (i, c.role))
            .collect()
    }

    pub fn live_count(&self) -> usize {
        self.cards.iter().filter(|c| !c.revealed).count()
    }

    pub fn is_alive(&self) -> bool {
        self.live_count() > 0
    }

    pub fn find_live_role(&self, role: Role) -> Option<usize> {
        self.cards
            .iter()
            .position(|c| !c.revealed && c.role == role)
    }

    pub fn reveal_slot(&mut self, slot: usize) -> Role {
        debug_assert!(!self.cards[slot].revealed);
        self.cards[slot].revealed = true;
        self.cards[slot].role
    }
}

#[derive(Clone, Debug)]
pub struct Roster {
    pub seats: Vec<User>,
}

impl Roster {
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    pub fn nick(&self, seat: usize) -> String {
        self.seats[seat].nick()
    }

    pub fn seat_of(&self, user: &User) -> Option<usize> {
        self.seats.iter().position(|u| u == user)
    }

    pub fn resolve_nick(&self, raw: &str, casemap: CaseMap) -> Option<usize> {
        let cleaned = raw
            .trim()
            .trim_start_matches('@')
            .trim_end_matches([':', ',']);
        (!cleaned.is_empty())
            .then(|| {
                self.seats
                    .iter()
                    .position(|u| casemap.str_eq(&u.nick(), cleaned))
            })
            .flatten()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardState {
    pub mode: GameMode,
    pub players: Vec<PlayerState>,
    pub deck: Vec<Role>,
    pub treasury: u32,
    pub current_turn: usize,
    pub turn_number: u32,
}

impl BoardState {
    pub fn new(
        mode: GameMode,
        players: Vec<PlayerState>,
        deck: Vec<Role>,
        treasury: u32,
        initial_turn: usize,
    ) -> Self {
        assert!((MIN_PLAYERS..=MAX_PLAYERS).contains(&players.len()));
        assert!(initial_turn < players.len());
        Self {
            mode,
            players,
            deck,
            treasury,
            current_turn: initial_turn,
            turn_number: 1,
        }
    }

    pub fn is_alive(&self, seat: usize) -> bool {
        self.players[seat].is_alive()
    }

    pub fn alive_seats(&self) -> Vec<usize> {
        (0..self.players.len())
            .filter(|&i| self.is_alive(i))
            .collect()
    }

    pub fn alive_count(&self) -> usize {
        self.players.iter().filter(|p| p.is_alive()).count()
    }

    pub fn winner(&self) -> Option<usize> {
        let mut iter = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_alive())
            .map(|(i, _)| i);
        let first = iter.next()?;
        iter.next().is_none().then_some(first)
    }

    /// Official *Coup: Reformation* rule: targeting & Foreign Aid block restrictions lift
    /// whenever all surviving players share the same Allegiance.
    pub fn all_alive_same_allegiance(&self) -> bool {
        if !self.mode.reformation {
            return true;
        }
        let mut first = None;
        self.players
            .iter()
            .filter(|p| p.is_alive())
            .filter_map(|p| p.allegiance)
            .all(|a| *first.get_or_insert(a) == a)
    }

    pub fn can_target(&self, actor: usize, target: usize) -> bool {
        actor != target
            && target < self.players.len()
            && self.is_alive(target)
            && (!self.mode.reformation
                || self.all_alive_same_allegiance()
                || self.players[actor].allegiance != self.players[target].allegiance)
    }

    pub fn can_block_foreign_aid(&self, actor: usize, blocker: usize) -> bool {
        self.can_target(blocker, actor)
    }

    /// Official rule: you may always challenge any living player, even a teammate.
    pub fn can_challenge(&self, claimant: usize, challenger: usize) -> bool {
        challenger != claimant && challenger < self.players.len() && self.is_alive(challenger)
    }

    pub fn must_coup(&self, seat: usize) -> bool {
        self.players[seat].coins >= MANDATORY_COUP_COINS
    }

    pub fn valid_targets(&self, actor: usize) -> Vec<usize> {
        (0..self.players.len())
            .filter(|&t| self.can_target(actor, t))
            .collect()
    }

    /// Returns `slots` to the Court Deck, shuffles, and draws replacements in place.
    pub fn swap_live_slots(&mut self, seat: usize, slots: &[usize]) -> Vec<Role> {
        for &slot in slots {
            self.deck.push(self.players[seat].cards[slot].role);
        }
        fastrand::shuffle(&mut self.deck);
        slots
            .iter()
            .map(|&slot| {
                let role = self.deck.pop().expect("Court Deck has replacement cards");
                self.players[seat].cards[slot].role = role;
                role
            })
            .collect()
    }

    /// Official Inquisitor Examine forced exchange: draw a replacement BEFORE returning the
    /// examined card to the Court Deck.
    pub fn force_replace_examined_card(&mut self, target: usize, slot: usize) -> (Role, Role) {
        let old = self.players[target].cards[slot].role;
        fastrand::shuffle(&mut self.deck);
        let new_role = self.deck.pop().expect("Court Deck has replacement card");
        self.players[target].cards[slot].role = new_role;
        self.deck.push(old);
        fastrand::shuffle(&mut self.deck);
        (old, new_role)
    }

    pub fn advance_turn(&mut self) -> Option<usize> {
        if self.alive_count() <= 1 {
            return None;
        }
        let n = self.players.len();
        let next = (1..=n)
            .map(|s| (self.current_turn + s) % n)
            .find(|&c| self.is_alive(c))?;
        self.current_turn = next;
        self.turn_number += 1;
        Some(next)
    }

    pub(crate) fn format_seats(&self, roster: &Roster, show_lost_roles: bool) -> Text {
        let parts: Vec<Text> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let nick = roster.nick(i);
                if !p.is_alive() {
                    if !show_lost_roles {
                        return irc_format!("💀 {}", nick.grey());
                    }
                    let dead: Vec<Text> = p.cards.iter().map(|c| c.role.styled()).collect();
                    return irc_format!("💀 {} ({})", nick.grey(), Text::join(&dead, "/".grey()));
                }
                let marker = if i == self.current_turn { "▶ " } else { "" };
                let team = p
                    .allegiance
                    .map_or_else(Text::default, |a| irc_format!("[{}] ", a.short_styled()));
                let lost: Vec<Text> = if show_lost_roles {
                    p.cards
                        .iter()
                        .filter(|c| c.revealed)
                        .map(|c| c.role.styled())
                        .collect()
                } else {
                    Vec::new()
                };
                let info: Text = if lost.is_empty() {
                    format!("{}♥", p.live_count()).into()
                } else {
                    irc_format!(
                        "{}♥, lost {}",
                        p.live_count(),
                        Text::join(&lost, "/".grey())
                    )
                };
                irc_format!("{marker}{team}{} ({}c, {info})", nick.bold(), p.coins)
            })
            .collect();
        Text::join(&parts, " · ".grey())
    }

    pub fn format_board_banner(&self, roster: &Roster) -> Text {
        let extra = if self.mode.reformation {
            irc_format!(
                " · 🏛️ Treasury: {} · 🃏 Deck: {}",
                format!("{}c", self.treasury).bold().yellow(),
                self.deck.len(),
            )
        } else {
            irc_format!(" · 🃏 Deck: {}", self.deck.len())
        };
        irc_format!(
            "👑 Turn {} — {}{extra}",
            self.turn_number,
            self.format_seats(roster, true)
        )
    }
}

#[cfg(test)]
#[path = "state_test.rs"]
mod test;

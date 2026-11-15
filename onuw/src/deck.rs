//! Lobby [`DeckSpec`] builder, custom `!add`/`!remove` token parser, and card dealing.

use ircgm::prelude::*;

use crate::board::{format_role_slice_styled, BoardCards, Card, Roster, SeatInfo};
use crate::role::Role;
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::{find_role, ALL_ROLES};
use crate::scenarios::{find_scenario, Scenario, ScenarioPack};

/// Underlying base deck before applying host `!add` / `!remove` overrides.
#[derive(Clone, Copy, Debug)]
pub enum DeckBase {
    /// Auto-selects the pack's default scenario matching the final player count (`3..=10`).
    Pack(ScenarioPack),
    /// Pinned to a specific official scenario (enforcing `scen.player_range`).
    Scenario(&'static Scenario),
    /// Pure custom deck (`!clear`), filling any remaining slots with `Villager`.
    Custom,
}

impl Default for DeckBase {
    fn default() -> Self {
        Self::Pack(ScenarioPack::Base)
    }
}

/// Mutable lobby deck configuration supporting expansion packs, official scenarios,
/// and custom `!add` / `!remove` / `!clear` / `!reset` overrides.
#[derive(Clone, Debug, Default)]
pub struct DeckSpec {
    pub base: DeckBase,
    pub added: Vec<&'static dyn Role>,
    pub removed: Vec<&'static dyn Role>,
}

impl DeckSpec {
    pub fn from_pack(pack: ScenarioPack) -> Self {
        Self {
            base: DeckBase::Pack(pack),
            added: Vec::new(),
            removed: Vec::new(),
        }
    }

    pub fn from_scenario(scenario: &'static Scenario) -> Self {
        Self {
            base: DeckBase::Scenario(scenario),
            added: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Resolves either an expansion pack (`base`, `daybreak`, `vampire`, `epic_battle`, `alien`,
    /// `villains`) OR any of the 59 official preset scenarios by ID, alias (`chaos`, `anarchy`, etc.), or name.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Some(Self::default());
        }
        if let Some(pack) = ScenarioPack::parse(trimmed) {
            return Some(Self::from_pack(pack));
        }
        find_scenario(trimmed).map(Self::from_scenario)
    }

    pub fn set_pack(&mut self, pack: ScenarioPack) {
        self.base = DeckBase::Pack(pack);
        self.added.clear();
        self.removed.clear();
    }

    pub fn set_scenario(&mut self, scenario: &'static Scenario) {
        self.base = DeckBase::Scenario(scenario);
        self.added.clear();
        self.removed.clear();
    }

    pub fn clear_to_custom(&mut self) {
        self.base = DeckBase::Custom;
        self.added.clear();
        self.removed.clear();
    }

    pub fn reset_overrides(&mut self) {
        if matches!(self.base, DeckBase::Custom) {
            self.base = DeckBase::default();
        }
        self.added.clear();
        self.removed.clear();
    }

    /// Valid `(min_players, max_players)` range for this deck configuration.
    pub fn player_range(&self) -> (usize, usize) {
        match self.base {
            DeckBase::Scenario(scen) => scen.player_range,
            DeckBase::Pack(_) | DeckBase::Custom => (3, 10),
        }
    }

    pub fn default_extra_evil_card(&self) -> Card {
        match self.base {
            DeckBase::Pack(pack) => pack.default_extra_evil_card(),
            DeckBase::Scenario(scen) => scen.pack.default_extra_evil_card(),
            DeckBase::Custom => Card::new(&WEREWOLF),
        }
    }

    /// Adds `count` copies of `role` (in multiples of 2 if `requires_pair_in_deck()`).
    pub fn add_role(&mut self, role: &'static dyn Role, count: usize) -> usize {
        let unit = if role.requires_pair_in_deck() {
            count.max(1).div_ceil(2) * 2
        } else {
            count.max(1)
        };
        let mut remaining_to_add = unit;
        while remaining_to_add > 0 {
            if let Some(pos) = self.removed.iter().position(|r| r.id() == role.id()) {
                self.removed.remove(pos);
                remaining_to_add -= 1;
            } else {
                break;
            }
        }
        for _ in 0..remaining_to_add {
            self.added.push(role);
        }
        unit
    }

    /// Removes `count` copies of `role` (in multiples of 2 if `requires_pair_in_deck()`).
    pub fn remove_role(&mut self, role: &'static dyn Role, count: usize) -> usize {
        let unit = if role.requires_pair_in_deck() {
            count.max(1).div_ceil(2) * 2
        } else {
            count.max(1)
        };
        let mut remaining_to_remove = unit;
        while remaining_to_remove > 0 {
            if let Some(pos) = self.added.iter().rposition(|r| r.id() == role.id()) {
                self.added.remove(pos);
                remaining_to_remove -= 1;
            } else {
                break;
            }
        }
        if !matches!(self.base, DeckBase::Custom) {
            for _ in 0..remaining_to_remove {
                self.removed.push(role);
            }
        }
        unit
    }

    pub fn mode_label(&self) -> String {
        let base = match self.base {
            DeckBase::Custom => return "Custom Deck".to_owned(),
            DeckBase::Pack(pack) => pack.label().to_owned(),
            DeckBase::Scenario(scen) => format!("{} ({})", scen.name, scen.pack.label()),
        };
        if self.added.is_empty() && self.removed.is_empty() {
            base
        } else {
            format!("{base} + custom")
        }
    }

    /// Builds an exact `(player_count + 3)`-card deck for `player_count` players, validating
    /// scenario player range, custom card counts, and pair constraints.
    pub fn build_for_players(
        &self,
        player_count: usize,
    ) -> std::result::Result<Vec<&'static dyn Role>, String> {
        let target = player_count + 3;
        if self.added.len() > target {
            return Err(format!(
                "Too many custom added cards ({}) for {player_count} players (deck size is {target}). Remove {} card(s) or wait for more players!",
                self.added.len(),
                self.added.len() - target
            ));
        }

        let mut base_cards: Vec<&'static dyn Role> = match self.base {
            DeckBase::Custom => Vec::new(),
            DeckBase::Scenario(scen) => scen.build_for_players(player_count)?,
            DeckBase::Pack(pack) => pack
                .default_scenario_for(player_count)
                .build_for_players(player_count)?,
        };

        for &rem in &self.removed {
            if let Some(pos) = base_cards.iter().rposition(|r| r.id() == rem.id()) {
                base_cards.remove(pos);
            }
        }
        enforce_even_pairs(&mut base_cards);

        let mut deck: Vec<&'static dyn Role> = self.added.clone();
        for card in base_cards {
            if deck.len() >= target {
                break;
            }
            if card.requires_pair_in_deck() {
                let cur_count = deck.iter().filter(|r| r.id() == card.id()).count();
                if cur_count % 2 == 1 || deck.len() + 2 <= target {
                    deck.push(card);
                }
            } else {
                deck.push(card);
            }
        }
        enforce_even_pairs(&mut deck);

        while deck.len() < target {
            deck.push(&VILLAGER);
        }
        deck.truncate(target);

        for &role in ALL_ROLES {
            if role.requires_pair_in_deck() {
                let count = deck.iter().filter(|&&r| r.id() == role.id()).count();
                if count % 2 != 0 {
                    return Err(format!(
                        "Role {} must be included in pairs (found {count}).",
                        role.name()
                    ));
                }
            }
        }

        Ok(deck)
    }

    pub fn preview_for_players(&self, player_count: usize) -> Text {
        if self.added.is_empty() && self.removed.is_empty() {
            if let DeckBase::Scenario(scen) = self.base {
                let sep = " · ".grey();
                return irc_format!(
                    "{}{sep}{}{sep}{}",
                    scen.name.bold(),
                    scen.difficulty.styled_label(),
                    scen.preview_cards(player_count),
                );
            }
        }
        let (min_p, max_p) = self.player_range();
        let effective_n = player_count.clamp(min_p, max_p);
        match self.build_for_players(effective_n) {
            Ok(deck) => irc_format!(
                "{} ({effective_n}p, {} cards): {}",
                self.mode_label().bold(),
                deck.len(),
                format_role_slice_styled(&deck),
            ),
            Err(err) => format!("⚠️ Invalid deck for {effective_n}p: {err}").yellow(),
        }
    }

    /// Shuffles and deals `(N + 3)` cards across `players` and the 3 center slots.
    pub fn deal(&self, players: &[User]) -> std::result::Result<(Roster, BoardCards), String> {
        let deck = self.build_for_players(players.len())?;
        let mut dealt_roles = deck.clone();
        fastrand::shuffle(&mut dealt_roles);

        let player_roles: Vec<_> = dealt_roles.drain(..players.len()).collect();
        let center: [Card; 3] = [
            Card::new(dealt_roles[0]),
            Card::new(dealt_roles[1]),
            Card::new(dealt_roles[2]),
        ];
        let player_cards: Vec<Card> = player_roles.iter().copied().map(Card::new).collect();
        let seats: Vec<SeatInfo> = players
            .iter()
            .zip(player_roles)
            .map(|(u, role)| SeatInfo {
                user: u.clone(),
                initial_role: role,
            })
            .collect();

        let roster = Roster { seats, deck };
        let board = BoardCards::new(player_cards, center, self.default_extra_evil_card());
        Ok((roster, board))
    }
}

fn enforce_even_pairs(cards: &mut Vec<&'static dyn Role>) {
    for &role in ALL_ROLES {
        if role.requires_pair_in_deck() {
            let count = cards.iter().filter(|&&r| r.id() == role.id()).count();
            if count % 2 == 1 {
                if let Some(pos) = cards.iter().rposition(|r| r.id() == role.id()) {
                    cards.remove(pos);
                }
            }
        }
    }
}

/// Parses a whitespace- or comma-separated role list supporting `2x werewolf` / `werewolf*2` multipliers.
pub fn parse_role_spec_tokens(
    raw: &str,
) -> std::result::Result<Vec<(&'static dyn Role, usize)>, String> {
    let mut out = Vec::new();
    for chunk in raw.split(',') {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace().peekable();
        if let Some(role) = find_role(trimmed) {
            out.push((role, 1));
            continue;
        }
        if let Some(&first) = parts.peek() {
            if let Some(mult) = parse_multiplier_prefix(first) {
                parts.next();
                let rest: String = parts.collect::<Vec<_>>().join(" ");
                if let Some(role) = find_role(&rest) {
                    out.push((role, mult));
                    continue;
                }
                return Err(format!("Unknown role '{rest}' (try !roles)."));
            }
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let mut i = 0;
        while i < words.len() {
            let w = words[i];
            if let Some(mult) = parse_multiplier_prefix(w) {
                i += 1;
                if i >= words.len() {
                    return Err(format!("Missing role name after '{w}'."));
                }
                if i + 1 < words.len() {
                    let two = format!("{} {}", words[i], words[i + 1]);
                    if let Some(role) = find_role(&two) {
                        out.push((role, mult));
                        i += 2;
                        continue;
                    }
                }
                if let Some(role) = find_role(words[i]) {
                    out.push((role, mult));
                    i += 1;
                    continue;
                }
                return Err(format!("Unknown role '{}' (try !roles).", words[i]));
            }

            if let Some((mult, name_part)) = split_inline_multiplier(w) {
                if let Some(role) = find_role(name_part) {
                    out.push((role, mult));
                    i += 1;
                    continue;
                }
            }

            if i + 2 < words.len() {
                let three = format!("{} {} {}", words[i], words[i + 1], words[i + 2]);
                if let Some(role) = find_role(&three) {
                    out.push((role, 1));
                    i += 3;
                    continue;
                }
            }
            if i + 1 < words.len() {
                let two = format!("{} {}", words[i], words[i + 1]);
                if let Some(role) = find_role(&two) {
                    out.push((role, 1));
                    i += 2;
                    continue;
                }
            }
            if let Some(role) = find_role(w) {
                out.push((role, 1));
                i += 1;
            } else {
                return Err(format!("Unknown role '{w}' (try !roles)."));
            }
        }
    }
    if out.is_empty() {
        return Err(
            "Specify at least one role name (e.g. !add tanner or !add 2x werewolf).".into(),
        );
    }
    Ok(out)
}

fn parse_multiplier_prefix(tok: &str) -> Option<usize> {
    let lower = tok.to_ascii_lowercase();
    let digits = lower.strip_suffix('x')?;
    let n = digits.parse::<usize>().ok()?;
    (1..=13).contains(&n).then_some(n)
}

fn split_inline_multiplier(tok: &str) -> Option<(usize, &str)> {
    if let Some((num, rest)) = tok.split_once('x').or_else(|| tok.split_once('*')) {
        if let Ok(n) = num.parse::<usize>() {
            if (1..=13).contains(&n) && !rest.is_empty() {
                return Some((n, rest));
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "deck_test.rs"]
mod test;

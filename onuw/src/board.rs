//! Physical [`Card`], [`BoardCards`] tabletop state, and [`Roster`] player ring.

use std::collections::{HashMap, HashSet};
use std::fmt;

use ircgm::prelude::*;

use crate::artifacts::Artifact;
use crate::marks::Mark;
use crate::role::{GameSet, Role, RoleTag, Team};

/// Cosmic signal frequency tuned by the Alien `Oracle` (read by `Psychic`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OracleOmen {
    #[default]
    Normal,
    /// Doubles `Psychic` peeks to both neighbors.
    AmplifiedSignals,
    /// Redirects `Psychic` cosmic signals to right neighbor.
    QuantumStatic,
}

/// A physical card on the table: `face` is what is printed on the cardboard when peeked or flipped,
/// `became` records any mid-night copy/transformation (`Copycat`, `Doppelgänger`, `Paranormal Investigator`),
/// and `face_up` tracks whether the card was flipped face-up (`Revealer`) so face-up state travels with swaps.
#[derive(Clone, Copy)]
pub struct Card {
    pub face: &'static dyn Role,
    pub became: Option<&'static dyn Role>,
    pub face_up: bool,
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.became {
            Some(r) => write!(f, "{}({})", self.face.name(), r.name()),
            None => write!(f, "{}", self.face.name()),
        }
    }
}

impl Card {
    pub const fn new(face: &'static dyn Role) -> Self {
        Self {
            face,
            became: None,
            face_up: false,
        }
    }

    pub fn transform(&mut self, became: &'static dyn Role) {
        self.became = Some(became);
    }

    /// The physical printed face on this card when peeked or flipped during Night.
    pub fn face_role(self) -> &'static dyn Role {
        self.face
    }

    /// The role this card counts as for team, ability, and victory rules at Dawn.
    pub fn effective_role(self) -> &'static dyn Role {
        self.became
            .or_else(|| self.face.copycat_fallback())
            .unwrap_or(self.face)
    }

    pub fn display_for_peek(self) -> Text {
        self.face.styled_name()
    }

    pub fn display_reveal(self) -> Text {
        match self.became {
            Some(r) => irc_format!("{} ({})", self.face.styled_name(), r.styled_name()),
            None => self.face.styled_name(),
        }
    }
}

#[derive(Clone)]
pub struct SeatInfo {
    pub user: User,
    pub initial_role: &'static dyn Role,
}

pub struct Roster {
    pub seats: Vec<SeatInfo>,
    pub deck: Vec<&'static dyn Role>,
}

impl Roster {
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }

    pub fn initial_holders(&self, role: &'static dyn Role) -> Vec<usize> {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (s.initial_role.id() == role.id()).then_some(i))
            .collect()
    }

    pub fn format_players(&self) -> String {
        self.seats
            .iter()
            .map(|s| s.user.nick())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Formats the seating ring explicitly (`"Alice ↔ Bob ↔ Carol (wraps)"`) so neighbor-based roles
    /// (`left` / `right`) are unambiguous on IRC.
    pub fn format_table_circle(&self) -> String {
        let chain = self
            .seats
            .iter()
            .map(|s| s.user.nick())
            .collect::<Vec<_>>()
            .join(" ↔ ");
        format!("{chain} (wraps)")
    }

    pub fn format_players_marking_shields(&self, shielded: &HashSet<usize>) -> String {
        self.seats
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if shielded.contains(&i) {
                    format!("{}🛡", s.user.nick())
                } else {
                    s.user.nick()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn format_indices(&self, indices: &[usize]) -> String {
        indices
            .iter()
            .map(|&i| self.seats[i].user.nick())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn format_deck(&self) -> String {
        format_role_slice(&self.deck)
    }

    pub fn format_deck_styled(&self) -> Text {
        format_role_slice_styled(&self.deck)
    }

    pub fn deck_has_tag(&self, tag: RoleTag) -> bool {
        self.deck.iter().any(|r| r.tags().contains(&tag))
    }

    pub fn deck_has(&self, role: &'static dyn Role) -> bool {
        self.deck_has_id(role.id())
    }

    fn deck_has_id(&self, id: &str) -> bool {
        self.deck.iter().any(|r| r.id() == id)
    }

    pub fn deck_has_marks(&self) -> bool {
        self.deck_has_tag(RoleTag::PlacesMarks)
    }

    pub fn deck_has_shields(&self) -> bool {
        self.deck_has_id("sentinel")
    }

    /// True iff `self.deck` contains a death-win spoiler (`Tanner`, `Mad Scientist`, `Synthetic Alien`, or `Curator`'s Cudgel).
    pub fn deck_has_tanner_hazard(&self) -> bool {
        self.deck.iter().any(|r| {
            matches!(
                r.id(),
                "tanner" | "mad_scientist" | "synthetic_alien" | "curator"
            )
        })
    }

    /// Human-readable phrase naming ONLY the non-Village categories actually present in `self.deck`.
    pub fn describe_non_village_roles(&self) -> String {
        let mut labels: Vec<&'static str> = Vec::new();
        let mut push_unique = |s: &'static str| {
            if !labels.contains(&s) {
                labels.push(s);
            }
        };

        if self.deck_has_tag(RoleTag::WolfPack)
            || self.deck_has_tag(RoleTag::DreamWolfThumb)
            || self.deck_has_id("curator")
        {
            push_unique("Werewolf");
        }
        if self.deck_has_tag(RoleTag::VampirePack) {
            push_unique("Vampire");
        }
        if self
            .deck
            .iter()
            .any(|r| r.team() == Team::Alien || (r.tags().contains(&RoleTag::AlienPack) && r.team() == Team::Werewolf))
        {
            push_unique("Alien");
        }
        if self.deck_has_tag(RoleTag::VillainPack)
            || self
                .deck
                .iter()
                .any(|r| r.team() == Team::SuperVillain || (r.team() == Team::Werewolf && r.game() == GameSet::SuperVillains))
        {
            push_unique("Super Villain");
        }
        if self
            .deck
            .iter()
            .any(|r| r.is_minion() && r.team() == Team::Werewolf)
        {
            push_unique("Minion");
        }
        if self
            .deck
            .iter()
            .any(|r| r.is_minion() && r.team() == Team::Vampire)
        {
            push_unique("Renfield");
        }
        for &r in &self.deck {
            if r.team() == Team::Independent && !r.is_copycat() {
                push_unique(r.name());
            }
        }
        if self.deck_has_id("curator") {
            push_unique("Tanner");
        }

        if labels.is_empty() {
            return "an Evil role".to_owned();
        }
        let article = match labels[0].chars().next().unwrap_or('W') {
            'A' | 'E' | 'I' | 'O' | 'U' => "an",
            _ => "a",
        };
        match labels.len() {
            1 => format!("{article} {}", labels[0]),
            2 => format!("{article} {} or {}", labels[0], labels[1]),
            _ => {
                let last = labels.last().copied().unwrap_or("");
                let head = &labels[..labels.len() - 1];
                format!("{article} {}, or {last}", head.join(", "))
            }
        }
    }

    /// Resolves an IRC nickname (matched via [`User::eq_nick`] under the bot's active `CaseMap`)
    /// to a 0-based player index.
    pub fn resolve_nick(&self, nick: &str) -> Option<usize> {
        let clean = nick.trim();
        if clean.is_empty() {
            return None;
        }
        self.seats.iter().position(|s| s.user.eq_nick(clean))
    }

    pub fn neighbors(&self, seat: usize) -> (usize, usize) {
        let n = self.seats.len();
        ((seat + n - 1) % n, (seat + 1) % n)
    }

    /// Parses `"1"` / `"#1"` / `"c1"` / `"center 1"` into `Some(0..3)`.
    pub fn parse_center_slot(raw: &str) -> Option<usize> {
        let norm = raw.trim().to_ascii_lowercase();
        let rest = norm
            .strip_prefix("center")
            .or_else(|| norm.strip_prefix('c'))
            .unwrap_or(&norm)
            .trim();
        let rest = rest.strip_prefix('#').unwrap_or(rest);
        match rest {
            "1" => Some(0),
            "2" => Some(1),
            "3" => Some(2),
            _ => None,
        }
    }

    pub fn is_pass(raw: &str) -> bool {
        matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "pass" | "skip" | "none" | "!pass" | "!skip"
        )
    }
}

pub fn format_role_slice(roles: &[&'static dyn Role]) -> String {
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for &r in roles {
        if let Some(entry) = counts.iter_mut().find(|(name, _)| *name == r.name()) {
            entry.1 += 1;
        } else {
            counts.push((r.name(), 1));
        }
    }
    counts
        .into_iter()
        .map(|(name, n)| {
            if n > 1 {
                format!("{n}x {name}")
            } else {
                name.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_role_slice_styled(roles: &[&'static dyn Role]) -> Text {
    let mut counts: Vec<(&'static dyn Role, usize)> = Vec::new();
    for &r in roles {
        if let Some(entry) = counts.iter_mut().find(|(role, _)| role.id() == r.id()) {
            entry.1 += 1;
        } else {
            counts.push((r, 1));
        }
    }
    let parts: Vec<Text> = counts
        .into_iter()
        .map(|(role, n)| {
            if n > 1 {
                irc_format!("{n}x {}", role.styled_name())
            } else {
                role.styled_name()
            }
        })
        .collect();
    Text::join(&parts, ", ".grey())
}

/// Complete tabletop state across Dusk, Night, Day, and Dawn.
#[derive(Clone, Debug)]
pub struct BoardCards {
    pub players: Vec<Card>,
    pub center: [Card; 3],
    /// Extra center Werewolf card used by `Alpha Wolf`.
    pub extra_evil_card: Card,
    /// Extra perpendicular Villain card (`Henchman`) used by `Temptress` / `Rapscallion`.
    pub extra_villain_card: Card,
    /// Seats protected by `Sentinel`'s shield token (cannot be viewed, moved, flipped, or artifacted).
    pub shielded: HashSet<usize>,
    /// Per-seat `Mark` tokens (Vampire expansion; initialized to `Mark::Clarity`).
    pub marks: Vec<Mark>,
    /// Per-seat face-down `Artifact` tokens placed during Night (`Curator`).
    pub artifacts: HashMap<usize, Artifact>,
    /// Seats that viewed or moved any card tonight (read by Daybreak's `Aura Seer`).
    pub viewed_or_moved_actors: HashSet<usize>,
    /// Seats absorbed into the `Blob` (Alien expansion).
    pub blob_seats: HashSet<usize>,
    /// Cosmic signal frequency tuned by the `Oracle` (read by `Psychic`).
    pub oracle_omen: OracleOmen,
}

impl BoardCards {
    pub fn new(players: Vec<Card>, center: [Card; 3], default_evil_card: Card) -> Self {
        let extra_villain_card = if default_evil_card.face.team() == Team::SuperVillain {
            default_evil_card
        } else {
            Card::new(&crate::roles::super_villains::henchman::HENCHMAN)
        };
        Self::with_extra_cards(players, center, default_evil_card, extra_villain_card)
    }

    pub fn with_extra_cards(
        players: Vec<Card>,
        center: [Card; 3],
        extra_evil_card: Card,
        extra_villain_card: Card,
    ) -> Self {
        let n = players.len();
        Self {
            players,
            center,
            extra_evil_card,
            extra_villain_card,
            shielded: HashSet::new(),
            marks: vec![Mark::Clarity; n],
            artifacts: HashMap::new(),
            viewed_or_moved_actors: HashSet::new(),
            blob_seats: HashSet::new(),
            oracle_omen: OracleOmen::Normal,
        }
    }

    pub fn is_shielded(&self, seat: usize) -> bool {
        self.shielded.contains(&seat)
    }

    pub fn flip_player_face_up(&mut self, seat: usize) {
        self.players[seat].face_up = true;
    }

    pub fn is_player_face_up(&self, seat: usize) -> bool {
        self.players[seat].face_up
    }

    pub fn flip_center_face_up(&mut self, c_idx: usize) {
        self.center[c_idx].face_up = true;
    }

    pub fn is_center_face_up(&self, c_idx: usize) -> bool {
        self.center[c_idx].face_up
    }

    /// True if `seat` holds a role-changing `Artifact` (`Claw`, `Brand`, `Cudgel`), which overrides
    /// both Dusk `Mark`s and printed/copied Dawn role abilities (`vote_protection`, `on_death`, `special_victory`).
    pub fn has_role_override_artifact(&self, seat: usize) -> bool {
        self.artifacts
            .get(&seat)
            .and_then(|tok| tok.team_override())
            .is_some()
    }

    pub fn record_card_interaction(&mut self, actor_seat: usize) {
        self.viewed_or_moved_actors.insert(actor_seat);
    }

    /// Computes a seat's final effective [`Team`] after applying Dusk `Mark`s first and Night `#11`
    /// role-changing `Artifact`s second (so `Artifact`s override `Mark`s).
    pub fn effective_team(&self, seat: usize) -> Team {
        let mut team = self.players[seat].effective_role().team();

        match self.marks[seat] {
            Mark::Vampire => team = Team::Vampire,
            Mark::Traitor => {
                let any_vamp = (0..self.players.len()).any(|i| {
                    self.marks[i] == Mark::Vampire
                        || self.players[i].effective_role().team() == Team::Vampire
                });
                let any_wolf = (0..self.players.len()).any(|i| {
                    self.artifacts.get(&i).and_then(|t| t.team_override()) == Some(Team::Werewolf)
                        || self.players[i].effective_role().team() == Team::Werewolf
                });
                let any_alien = (0..self.players.len())
                    .any(|i| self.players[i].effective_role().team() == Team::Alien);
                let any_villain = (0..self.players.len())
                    .any(|i| self.players[i].effective_role().team() == Team::SuperVillain);
                team = match team {
                    Team::Village => {
                        if any_vamp && !any_wolf && !any_alien && !any_villain {
                            Team::Vampire
                        } else if any_alien && !any_wolf && !any_vamp && !any_villain {
                            Team::Alien
                        } else if any_villain && !any_wolf && !any_vamp && !any_alien {
                            Team::SuperVillain
                        } else {
                            Team::Werewolf
                        }
                    }
                    Team::Werewolf | Team::Vampire | Team::Alien | Team::SuperVillain => {
                        Team::Village
                    }
                    Team::Independent => Team::Independent,
                };
            }
            _ => {}
        }

        if let Some(tok) = self.artifacts.get(&seat) {
            if let Some(override_team) = tok.team_override() {
                team = override_team;
            }
        }

        team
    }

    /// True if `seat` counts as a primary monster threat for `monster_team`
    /// (`Werewolf`, `Vampire`, `Alien`, or `SuperVillain`), respecting `Artifact` > `Mark` > `Card` precedence.
    pub fn is_monster_threat_for(&self, seat: usize, monster_team: Team) -> bool {
        if let Some(tok) = self.artifacts.get(&seat) {
            if let Some(override_team) = tok.team_override() {
                return override_team == monster_team;
            }
        }
        match self.marks[seat] {
            Mark::Vampire => return monster_team == Team::Vampire,
            Mark::Traitor => return false,
            _ => {}
        }
        let role = self.players[seat].effective_role();
        role.team() == monster_team && !role.is_minion()
    }

    /// True if `seat` counts as a primary Werewolf threat.
    pub fn is_wolf_threat(&self, seat: usize) -> bool {
        self.is_monster_threat_for(seat, Team::Werewolf)
    }

    /// True if `seat` counts as a primary Vampire threat (`Vampire`, `The Master`, `Count`, or `Mark::Vampire`).
    pub fn is_vampire_threat(&self, seat: usize) -> bool {
        self.is_monster_threat_for(seat, Team::Vampire)
    }

    /// True if `seat` counts as a primary Alien threat.
    pub fn is_alien_threat(&self, seat: usize) -> bool {
        self.is_monster_threat_for(seat, Team::Alien)
    }

    /// True if `seat` counts as a primary Super Villain threat.
    pub fn is_villain_threat(&self, seat: usize) -> bool {
        self.is_monster_threat_for(seat, Team::SuperVillain)
    }

    /// True if `seat` counts as any primary monster threat (`Werewolf`, `Vampire`, `Alien`, or `SuperVillain`).
    pub fn is_primary_threat(&self, seat: usize) -> bool {
        Team::MONSTER_TEAMS
            .iter()
            .any(|&t| self.is_monster_threat_for(seat, t))
    }
}

//! Physical [`Card`], guarded [`BoardCards`] tabletop state, [`BoardAccess`] zone masks,
//! [`BoardMut`] resolve mutator, [`BoardKey`] typed blackboard, and [`Roster`] player ring.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;

use ircgm::prelude::*;

use crate::artifacts::Artifact;
use crate::marks::Mark;
use crate::role::{EffectiveAlignment, EndgameModifier, NightFaction, ResolveCtx, Role, Team};
pub use crate::seat_set::{SeatIter, SeatSet};

/// Bitmask tracking which tabletop zones a [`crate::role::WakeStep`] reads during its prompt
/// or mutates at resolution time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoardAccess(pub u16);

impl BoardAccess {
    pub const NONE: u16 = 0;
    /// Card identity sitting in `players[seat]` (read mid-prompt by `Paranormal Investigator`; written by card swaps).
    pub const PLAYER_FACES: u16 = 1 << 0;
    /// In-place `Card::became` transformation (`Paranormal Investigator`, `Copycat`, `Doppelgänger`).
    pub const PLAYER_BECAME: u16 = 1 << 1;
    /// Center slots `0..3` (read mid-prompt by `Witch`; written by `Oracle`, `Witch`, `Drunk`).
    pub const CENTER_FACES: u16 = 1 << 2;
    /// `Card::face_up` state flipped by `Revealer`.
    pub const FACE_UP: u16 = 1 << 3;
    /// `Sentinel` shield tokens on player seats.
    pub const SHIELDS: u16 = 1 << 4;
    /// Vampire-expansion `Mark` tokens (read mid-prompt by `Count`; written by Dusk mark-placers, `Pickpocket`, `Gremlin`).
    pub const MARKS: u16 = 1 << 5;
    /// `Curator` face-down `Artifact` tokens on player seats.
    pub const ARTIFACTS: u16 = 1 << 6;
    /// Active waking role identities mutated by `Copycat` and `Doppelgänger`.
    pub const WAKING_ROLES: u16 = 1 << 7;
    /// Role-defined blackboard slot #0 (`OracleOmen`).
    pub const CUSTOM_0: u16 = 1 << 8;
    /// Role-defined blackboard slot #1 (`CenterWolfCard`, `ExtraVillainCard`, `ApprenticeSoloMark`).
    pub const CUSTOM_1: u16 = 1 << 9;

    /// Publicly visible tabletop markers (`🛡` Shield, `🔦` Face-up card, `🏺` Artifact token)
    /// seen by default by every role that opens its eyes (`eyes_open == true`).
    pub const VISIBLE_TABLE: u16 = Self::SHIELDS | Self::FACE_UP | Self::ARTIFACTS;

    /// Full read mask granted to synchronous resolution-time passive hooks.
    pub const ALL_READS: u16 = 0xFFFF;
}

/// Typed per-role extension state stored in [`BoardCards`]'s `TypeId` blackboard so `board.rs`
/// carries zero role-specific fields or expansion imports.
pub trait BoardKey: Clone + fmt::Debug + Send + Sync + 'static {
    const ACCESS: u16;
    fn init() -> Self;
}

trait BlackboardValue: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn BlackboardValue>;
}

impl<T: Clone + fmt::Debug + Send + Sync + 'static> BlackboardValue for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn BlackboardValue> {
        Box::new(self.clone())
    }
}

/// Outcome of [`BoardMut::copy_from_player`] (`Doppelgänger`).
pub struct CopiedPlayerCard {
    /// Physical card face seen by the Doppelgänger (`"Copycat"` when viewing a Copycat).
    pub seen_face: &'static dyn Role,
    /// Endgame role inherited on `Card.became` (Copycat's center role if viewing Copycat).
    pub endgame_role: &'static dyn Role,
    /// True when viewing a `Copycat` card (Doppelgänger sees only `"Copycat"` and does not wake again).
    pub is_opaque_copycat: bool,
}

/// A physical card on the table: `face` is what is printed on the cardboard when peeked or flipped,
/// `became` records any mid-night copy/transformation (`Copycat`, `Doppelgänger`, `Paranormal Investigator`),
/// and `face_up` tracks whether the card was flipped face-up (`Revealer`) so face-up state travels with swaps.
#[derive(Clone, Copy)]
pub struct Card {
    face: &'static dyn Role,
    became: Option<&'static dyn Role>,
    face_up: bool,
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

    /// The role this card counts as for team, ability, and victory rules during Voting.
    pub fn effective_role(self) -> &'static dyn Role {
        self.became.unwrap_or(self.face)
    }

    pub fn effective_role_ref(&self) -> &&'static dyn Role {
        self.became.as_ref().unwrap_or(&self.face)
    }

    pub fn is_face_up(self) -> bool {
        self.face_up
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

    pub fn all_seats(&self) -> SeatSet {
        SeatSet::all(self.seats.len())
    }

    pub fn initial_holders(&self, role: &'static dyn Role) -> SeatSet {
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

    pub fn format_seats(&self, seats: impl IntoIterator<Item = usize>) -> String {
        seats
            .into_iter()
            .map(|i| self.seats[i].user.nick())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn format_deck(&self) -> String {
        format_role_slice(&self.deck)
    }

    pub fn format_deck_styled(&self) -> Text {
        format_role_slice_styled(&self.deck)
    }

    pub fn deck_has(&self, role: &'static dyn Role) -> bool {
        self.deck.iter().any(|r| r.id() == role.id())
    }

    pub fn deck_has_marks(&self) -> bool {
        self.deck.iter().any(|r| r.places_marks())
    }

    /// Human-readable phrase naming ONLY the non-Village categories actually present in `self.deck`.
    pub fn describe_non_village_roles(&self) -> String {
        let mut labels: Vec<&'static str> = Vec::new();
        let mut push_unique = |s: &'static str| {
            if !labels.contains(&s) {
                labels.push(s);
            }
        };

        for &team in Team::MONSTER_TEAMS {
            if self.deck.iter().any(|r| {
                (r.team() == team && !r.is_minion())
                    || r.extra_non_village_labels().contains(&team.label())
            }) {
                push_unique(team.label());
            }
        }
        for &r in &self.deck {
            if r.team() == Team::Independent {
                push_unique(r.name());
            }
            for &extra in r.extra_non_village_labels() {
                if !Team::MONSTER_TEAMS.iter().any(|t| t.label() == extra) {
                    push_unique(extra);
                }
            }
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

/// Complete tabletop state across Dusk, Night, Day, and Voting.
///
/// All fields are private to `board` so Dusk/Night role code can only read or mutate tabletop state
/// through access-checked [`crate::role::PromptCtx`], [`crate::role::GroupPromptCtx`], and [`BoardMut`].
pub struct BoardCards {
    players: Vec<Card>,
    center: [Card; 3],
    shielded: SeatSet,
    marks: Vec<Mark>,
    artifacts: Vec<Option<Artifact>>,
    viewed_or_moved_actors: SeatSet,
    copied_from: Vec<Option<usize>>,
    blackboard: HashMap<TypeId, Box<dyn BlackboardValue>>,
}

impl Clone for BoardCards {
    fn clone(&self) -> Self {
        let blackboard = self
            .blackboard
            .iter()
            .map(|(&k, v)| (k, v.clone_box()))
            .collect();
        Self {
            players: self.players.clone(),
            center: self.center,
            shielded: self.shielded,
            marks: self.marks.clone(),
            artifacts: self.artifacts.clone(),
            viewed_or_moved_actors: self.viewed_or_moved_actors,
            copied_from: self.copied_from.clone(),
            blackboard,
        }
    }
}

impl fmt::Debug for BoardCards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoardCards")
            .field("players", &self.players)
            .field("center", &self.center)
            .field("shielded", &self.shielded)
            .field("marks", &self.marks)
            .field("artifacts", &self.artifacts)
            .field("viewed_or_moved_actors", &self.viewed_or_moved_actors)
            .field("copied_from", &self.copied_from)
            .finish()
    }
}

impl BoardCards {
    pub fn new(players: Vec<Card>, center: [Card; 3]) -> Self {
        let n = players.len();
        assert!(
            n <= SeatSet::MAX_SEATS,
            "BoardCards supports at most {} players (got {n})",
            SeatSet::MAX_SEATS
        );
        Self {
            players,
            center,
            shielded: SeatSet::EMPTY,
            marks: vec![Mark::Clarity; n],
            artifacts: vec![None; n],
            viewed_or_moved_actors: SeatSet::EMPTY,
            copied_from: vec![None; n],
            blackboard: HashMap::new(),
        }
    }

    pub(crate) fn copied_from(&self, seat: usize) -> Option<usize> {
        self.copied_from[seat]
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn player_card(&self, seat: usize) -> Card {
        self.players[seat]
    }

    pub fn center_card(&self, slot: usize) -> Card {
        self.center[slot]
    }

    pub fn mark(&self, seat: usize) -> Mark {
        self.marks[seat]
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn artifact(&self, seat: usize) -> Option<Artifact> {
        self.artifacts[seat]
    }

    pub fn shielded_seats(&self) -> SeatSet {
        self.shielded
    }

    pub fn artifact_seats(&self) -> SeatSet {
        self.artifacts
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.map(|_| i))
            .collect()
    }

    pub fn is_shielded(&self, seat: usize) -> bool {
        self.shielded.contains(seat)
    }

    pub fn is_player_face_up(&self, seat: usize) -> bool {
        self.players[seat].face_up
    }

    pub fn is_center_face_up(&self, c_idx: usize) -> bool {
        self.center[c_idx].face_up
    }

    pub fn get_key<K: BoardKey>(&self) -> K {
        self.blackboard
            .get(&TypeId::of::<K>())
            .and_then(|v| v.as_any().downcast_ref::<K>())
            .cloned()
            .unwrap_or_else(K::init)
    }

    fn set_key_raw<K: BoardKey>(&mut self, val: K) {
        self.blackboard.insert(TypeId::of::<K>(), Box::new(val));
    }

    fn entry_mut_raw<K: BoardKey>(&mut self) -> &mut K {
        self.blackboard
            .entry(TypeId::of::<K>())
            .or_insert_with(|| Box::new(K::init()))
            .as_any_mut()
            .downcast_mut::<K>()
            .expect("TypeId matches K")
    }

    fn format_seat_visible(&self, roster: &Roster, seat: usize) -> String {
        let mut s = roster.seats[seat].user.nick();
        if self.shielded.contains(seat) {
            s.push('🛡');
        }
        if self.players[seat].face_up {
            s.push_str(&format!(" [🔦 {}]", self.players[seat].face.name()));
        }
        if self.artifacts[seat].is_some() {
            s.push('🏺');
        }
        s
    }

    pub(crate) fn format_visible_players(&self, roster: &Roster) -> String {
        (0..roster.len())
            .map(|i| self.format_seat_visible(roster, i))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(crate) fn format_visible_seats(&self, roster: &Roster, seats: SeatSet) -> String {
        seats
            .into_iter()
            .map(|i| self.format_seat_visible(roster, i))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Returns a concise 1-line summary of publicly visible tabletop markers (`🛡`, `🔦 Role`, `🏺`),
    /// or `None` when no visible markers are currently on the table.
    pub(crate) fn visible_table_summary(&self, roster: &Roster) -> Option<Text> {
        let mut items: Vec<String> = Vec::new();
        for i in 0..roster.len() {
            let has_marker = self.shielded.contains(i)
                || self.players[i].face_up
                || self.artifacts[i].is_some();
            if has_marker {
                items.push(self.format_seat_visible(roster, i));
            }
        }
        if items.is_empty() {
            None
        } else {
            Some(irc_format!(
                "👀 {} {}",
                "On the table:".grey(),
                items.join(", "),
            ))
        }
    }

    /// True if `seat` holds a role-changing `Artifact` (`Claw`, `Brand`, `Cudgel`), which overrides
    /// both Dusk `Mark`s and printed/copied Voting role abilities.
    pub fn has_role_override_artifact(&self, seat: usize) -> bool {
        self.artifacts[seat].is_some_and(|tok| tok.replaces_role_card())
    }

    /// Seats currently holding `role` un-overridden by role-changing `Artifact`s or `Mark::Vampire`.
    pub fn active_holders(&self, role: &'static dyn Role) -> SeatSet {
        let target_id = role.id();
        (0..self.players.len())
            .filter(|&i| {
                !self.has_role_override_artifact(i)
                    && self.marks[i] != Mark::Vampire
                    && self.players[i].effective_role().id() == target_id
            })
            .collect()
    }

    /// Seats whose physical/copied card is `role` (un-overridden by role-changing `Artifact`s,
    /// even if holding `Mark::Vampire`).
    pub fn role_card_holders(&self, role: &'static dyn Role) -> SeatSet {
        let target_id = role.id();
        (0..self.players.len())
            .filter(|&i| {
                !self.has_role_override_artifact(i)
                    && self.players[i].effective_role().id() == target_id
            })
            .collect()
    }

    /// Active [`EndgameModifier`] stack for `seat` in priority order (`Role < Mark < Artifact`),
    /// omitting `Role` when suppressed by a role-replacing `Artifact`.
    pub fn seat_endgame_modifiers(&self, seat: usize) -> Vec<&dyn EndgameModifier> {
        let mut out: Vec<&dyn EndgameModifier> = Vec::with_capacity(3);
        if !self.has_role_override_artifact(seat) {
            out.push(self.players[seat].effective_role_ref());
        }
        out.push(&self.marks[seat]);
        if let Some(art) = &self.artifacts[seat] {
            out.push(art);
        }
        out
    }

    /// Folds `Role < Mark < Artifact` alignment for `seat`.
    pub fn effective_alignment(&self, seat: usize) -> EffectiveAlignment {
        let role_ref = self.players[seat].effective_role_ref();
        let mut align = EffectiveAlignment {
            team: role_ref.team(),
            threat_team: role_ref.threat_team(),
        };
        align = role_ref.adjust_alignment(seat, self, align);
        align = self.marks[seat].adjust_alignment(seat, self, align);
        if let Some(art) = &self.artifacts[seat] {
            align = art.adjust_alignment(seat, self, align);
        }
        align
    }

    /// Computes a seat's final effective [`Team`] after folding `Role < Mark < Artifact`.
    pub fn effective_team(&self, seat: usize) -> Team {
        self.effective_alignment(seat).team
    }

    /// True if `seat` counts as a primary monster threat for `monster_team`
    /// (`Werewolf`, `Vampire`, `Alien`, or `SuperVillain`), respecting `Artifact` > `Mark` > `Role` precedence.
    pub fn is_monster_threat_for(&self, seat: usize, monster_team: Team) -> bool {
        let role = self.players[seat].effective_role();
        let mut align = EffectiveAlignment {
            team: role.team(),
            threat_team: role.threat_team(),
        };
        align = self.marks[seat].adjust_alignment(seat, self, align);
        if let Some(art) = &self.artifacts[seat] {
            align = art.adjust_alignment(seat, self, align);
        }
        align.threat_team == Some(monster_team)
    }

    #[cfg(test)]
    pub(crate) fn set_mark_for_test(&mut self, seat: usize, mark: Mark) {
        self.marks[seat] = mark;
    }

    #[cfg(test)]
    pub(crate) fn insert_artifact_for_test(&mut self, seat: usize, artifact: Artifact) {
        self.artifacts[seat] = Some(artifact);
    }

    #[cfg(test)]
    pub(crate) fn swap_players_for_test(&mut self, a: usize, b: usize) {
        self.players.swap(a, b);
    }
}

// ============================================================================
// Private Snapshot Reader (`PromptBoard`) & Access-Checked `BoardMut`
// ============================================================================

/// Opaque pre-tier board snapshot handle held privately inside `PromptCtx` and `GroupPromptCtx`.
#[derive(Clone, Copy)]
pub(crate) struct PromptBoard<'a> {
    inner: &'a BoardCards,
}

impl<'a> PromptBoard<'a> {
    pub(crate) fn new(inner: &'a BoardCards) -> Self {
        Self { inner }
    }

    pub(crate) fn raw(self) -> &'a BoardCards {
        self.inner
    }
}

/// Guarded mutable borrow of [`BoardCards`] passed to Dusk/Night resolution closures.
///
/// Every method verifies the step's declared `reads` and `writes` masks and enforces
/// `Sentinel` shield protection + `Aura Seer` interaction tracking automatically.
pub struct BoardMut<'a> {
    inner: &'a mut BoardCards,
    step_name: &'static str,
    reads: u16,
    writes: u16,
}

impl<'a> BoardMut<'a> {
    pub(crate) fn new(
        inner: &'a mut BoardCards,
        step_name: &'static str,
        reads: u16,
        writes: u16,
    ) -> Self {
        Self {
            inner,
            step_name,
            reads,
            writes,
        }
    }

    #[inline]
    fn check_read(&self, mask: u16, zone: &str) {
        assert!(
            (self.reads & mask) == mask,
            "{}: undeclared read of {zone} — add .reads(BoardAccess::{zone}) to WakeStep",
            self.step_name
        );
    }

    #[inline]
    fn check_write(&self, mask: u16, zone: &str) {
        assert!(
            (self.writes & mask) == mask,
            "{}: undeclared write to {zone} — add .writes(BoardAccess::{zone}) to WakeStep",
            self.step_name
        );
    }

    pub fn is_shielded(&self, seat: usize) -> bool {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.inner.shielded.contains(seat)
    }

    pub fn unshielded_other_seats(&self, actor: usize) -> SeatSet {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        SeatSet::all(self.inner.players.len()).without(actor) - self.inner.shielded
    }

    pub fn mark_at(&self, seat: usize) -> Mark {
        self.inner.marks[seat]
    }

    pub fn card_has_night_faction(&self, seat: usize, faction: NightFaction) -> bool {
        self.inner.players[seat].effective_role().night_faction() == Some(faction)
    }

    pub fn aura_actors_excluding(&self, seat: usize) -> SeatSet {
        self.inner.viewed_or_moved_actors.without(seat)
    }

    pub fn get_key<K: BoardKey>(&self) -> K {
        self.inner.get_key::<K>()
    }

    pub fn set_key<K: BoardKey>(&mut self, val: K) {
        self.check_write(K::ACCESS, "BoardKey::ACCESS");
        self.inner.set_key_raw(val);
    }

    /// Records that `actor` viewed a card during an interactive prompt (`Paranormal Investigator`).
    pub fn record_prompt_peek(&mut self, actor: usize) {
        assert!(
            (self.reads & (BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES)) != 0,
            "{}: record_prompt_peek requires PLAYER_FACES or CENTER_FACES in .reads(...)",
            self.step_name
        );
        self.inner.viewed_or_moved_actors.insert(actor);
    }

    /// Records additional seats that viewed a peeked card (`Alien` group bio-scan).
    pub fn record_card_viewers(&mut self, viewers: SeatSet) {
        self.inner.viewed_or_moved_actors |= viewers;
    }

    /// Peeks `target`'s card if unshielded, automatically recording `actor` for `Aura Seer`.
    pub fn peek_player(&mut self, actor: usize, target: usize) -> Option<Card> {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        if self.inner.shielded.contains(target) {
            return None;
        }
        self.inner.viewed_or_moved_actors.insert(actor);
        Some(self.inner.players[target])
    }

    /// Peeks `center[slot]`, automatically recording `actor` for `Aura Seer`.
    pub fn peek_center(&mut self, actor: usize, slot: usize) -> Card {
        self.inner.viewed_or_moved_actors.insert(actor);
        self.inner.center[slot]
    }

    /// Swaps `players[a]` and `players[b]` if neither is shielded, returning `(old_a, old_b)`.
    pub fn swap_players(&mut self, actor: usize, a: usize, b: usize) -> Option<(Card, Card)> {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.check_write(BoardAccess::PLAYER_FACES, "PLAYER_FACES");
        if self.inner.shielded.contains(a) || self.inner.shielded.contains(b) {
            return None;
        }
        self.inner.viewed_or_moved_actors.insert(actor);
        let old_a = self.inner.players[a];
        let old_b = self.inner.players[b];
        self.inner.players.swap(a, b);
        Some((old_a, old_b))
    }

    /// Swaps `players[seat]` with `center[slot]` if `seat` is unshielded, returning `(old_player, old_center)`.
    pub fn swap_player_with_center(
        &mut self,
        actor: usize,
        seat: usize,
        slot: usize,
    ) -> Option<(Card, Card)> {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.check_write(
            BoardAccess::PLAYER_FACES | BoardAccess::CENTER_FACES,
            "PLAYER_FACES | CENTER_FACES",
        );
        if self.inner.shielded.contains(seat) {
            return None;
        }
        self.inner.viewed_or_moved_actors.insert(actor);
        let old_player = self.inner.players[seat];
        let old_center = self.inner.center[slot];
        std::mem::swap(&mut self.inner.players[seat], &mut self.inner.center[slot]);
        Some((old_player, old_center))
    }

    /// Swaps two center cards (`Oracle` `QuantumStatic`), recording `actor` for `Aura Seer`.
    pub fn swap_centers(&mut self, actor: usize, slot_a: usize, slot_b: usize) {
        self.check_write(BoardAccess::CENTER_FACES, "CENTER_FACES");
        self.inner.viewed_or_moved_actors.insert(actor);
        self.inner.center.swap(slot_a, slot_b);
    }

    /// Swaps `players[target]` with a role-owned extra card slot `K` (`Alpha Wolf`, `Temptress`)
    /// if `target` is unshielded, returning `(old_player_card, planted_card)`.
    pub fn swap_player_with_extra<K>(&mut self, actor: usize, target: usize) -> Option<(Card, Card)>
    where
        K: BoardKey + AsMut<Card>,
    {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.check_write(
            BoardAccess::PLAYER_FACES | K::ACCESS,
            "PLAYER_FACES | K::ACCESS",
        );
        if self.inner.shielded.contains(target) {
            return None;
        }
        self.inner.viewed_or_moved_actors.insert(actor);
        let old_player = self.inner.players[target];
        let planted = {
            let extra_slot = self.inner.entry_mut_raw::<K>().as_mut();
            let p = *extra_slot;
            *extra_slot = old_player;
            p
        };
        self.inner.players[target] = planted;
        Some((old_player, planted))
    }

    /// Rotates all unshielded seats other than `actor` one step (`Village Idiot`), returning how many cards moved.
    pub fn rotate_other_unshielded(&mut self, actor: usize, rotate_right: bool) -> usize {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.check_write(BoardAccess::PLAYER_FACES, "PLAYER_FACES");
        let ring: Vec<usize> = self.unshielded_other_seats(actor).into_iter().collect();
        if ring.len() >= 2 {
            self.inner.viewed_or_moved_actors.insert(actor);
            let snapshot: Vec<Card> = ring.iter().map(|&i| self.inner.players[i]).collect();
            let k = ring.len();
            for (pos, &dst_seat) in ring.iter().enumerate() {
                let src_pos = if rotate_right {
                    (pos + k - 1) % k
                } else {
                    (pos + 1) % k
                };
                self.inner.players[dst_seat] = snapshot[src_pos];
            }
            ring.len()
        } else {
            0
        }
    }

    /// Transforms `players[seat].became` in place (`Paranormal Investigator`).
    pub fn transform_in_place(&mut self, seat: usize, new_role: &'static dyn Role) {
        self.check_write(BoardAccess::PLAYER_BECAME, "PLAYER_BECAME");
        self.inner.players[seat].transform(new_role);
    }

    /// Copies `players[target]` onto `actor` (`Doppelgänger`), updating both card & waking identity.
    ///
    /// Per official ONUV rules, if `players[target]` is a physical `Copycat` card, `actor` sees only
    /// `"Copycat"`, silently inherits Copycat's center role on `Card.became` for endgame, and stays
    /// asleep for the rest of the night.
    pub fn copy_from_player(
        &mut self,
        actor: usize,
        target: usize,
        label_prefix: &str,
        rctx: &mut ResolveCtx<'_>,
    ) -> Option<CopiedPlayerCard> {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.check_write(
            BoardAccess::PLAYER_BECAME | BoardAccess::WAKING_ROLES,
            "PLAYER_BECAME | WAKING_ROLES",
        );
        if self.inner.shielded.contains(target) {
            return None;
        }
        let seen_face = self.inner.players[target].face_role();
        let endgame_role = self.inner.players[target].effective_role();
        let is_opaque_copycat = seen_face.id() == "copycat";
        self.inner.viewed_or_moved_actors.insert(actor);
        self.inner.copied_from[actor] = Some(target);
        self.inner.players[actor].transform(endgame_role);
        let waking_role = if is_opaque_copycat {
            seen_face
        } else {
            endgame_role
        };
        rctx.set_waking_identity(
            actor,
            waking_role,
            format!("{label_prefix}-{}", waking_role.name()),
        );
        Some(CopiedPlayerCard {
            seen_face,
            endgame_role,
            is_opaque_copycat,
        })
    }

    /// Copies `center[slot]` onto `actor` (`Copycat`), updating both card & waking identity
    /// (except when `center[slot]` is `Doppelgänger`, which stays asleep per official ONUV rules).
    pub fn copy_from_center(
        &mut self,
        actor: usize,
        slot: usize,
        label_prefix: &str,
        rctx: &mut ResolveCtx<'_>,
    ) -> &'static dyn Role {
        self.check_write(
            BoardAccess::PLAYER_BECAME | BoardAccess::WAKING_ROLES,
            "PLAYER_BECAME | WAKING_ROLES",
        );
        let copied = self.inner.center[slot].effective_role();
        self.inner.viewed_or_moved_actors.insert(actor);
        self.inner.players[actor].transform(copied);
        if !copied.is_doppelganger() {
            rctx.set_waking_identity(actor, copied, format!("{label_prefix}-{}", copied.name()));
        }
        copied
    }

    /// Places a `Sentinel` Shield token on `target`.
    pub fn place_shield(&mut self, target: usize) {
        self.check_write(BoardAccess::SHIELDS, "SHIELDS");
        self.inner.shielded.insert(target);
    }

    /// Peeks `target` (`Revealer`) and flips `players[target]` face-up iff it does not trigger flip-back,
    /// returning `Some((card, stayed_face_up))` when unshielded.
    pub fn reveal_player_if_village(&mut self, actor: usize, target: usize) -> Option<(Card, bool)> {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.check_write(BoardAccess::FACE_UP, "FACE_UP");
        if self.inner.shielded.contains(target) {
            return None;
        }
        self.inner.viewed_or_moved_actors.insert(actor);
        let card = self.inner.players[target];
        if card.face_role().triggers_pi_and_revealer() {
            Some((card, false))
        } else {
            self.inner.players[target].face_up = true;
            Some((card, true))
        }
    }

    /// Draws and places a random unused `Artifact` on `target` (`Curator`) if unshielded and un-artifacted.
    pub fn place_random_artifact(&mut self, target: usize) -> Option<Artifact> {
        self.check_read(
            BoardAccess::SHIELDS | BoardAccess::ARTIFACTS,
            "SHIELDS | ARTIFACTS",
        );
        self.check_write(BoardAccess::ARTIFACTS, "ARTIFACTS");
        if self.inner.shielded.contains(target) || self.inner.artifacts[target].is_some() {
            return None;
        }
        let art = Artifact::random_unused(&self.inner.artifacts);
        self.inner.artifacts[target] = Some(art);
        Some(art)
    }

    /// Places `mark` on `target` (`Vampire`, `Count`, `Renfield`, `Cupid`, `Diseased`, `Instigator`, `Priest`, `Assassin`).
    pub fn set_mark(&mut self, target: usize, mark: Mark) {
        self.check_write(BoardAccess::MARKS, "MARKS");
        self.inner.marks[target] = mark;
    }

    /// Swaps `marks[a]` and `marks[b]` (`Pickpocket`, `Gremlin`), returning `(old_a, old_b)`.
    pub fn swap_marks(&mut self, a: usize, b: usize) -> (Mark, Mark) {
        self.check_write(BoardAccess::MARKS, "MARKS");
        let old_a = self.inner.marks[a];
        let old_b = self.inner.marks[b];
        self.inner.marks.swap(a, b);
        (old_a, old_b)
    }
}

//! [`Role`] trait, [`RoleInfo`], [`NightFaction`], [`WakeStep`] schedule metadata,
//! [`EndgameModifier`] pipeline, and access-checked Dusk/Night hook contexts.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use ircgm::prelude::*;
use ircgm::{CaseMap, IntoAskMatch};
use tokio::time::Instant;

use crate::board::{BoardAccess, BoardCards, BoardKey, BoardMut, Card, PromptBoard, Roster, SeatSet};
use crate::marks::Mark;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Which One Night Ultimate box a role belongs to (mirrors `src/roles/<game>/`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameSet {
    Base,
    Daybreak,
    Vampire,
    Alien,
    SuperVillains,
}

impl GameSet {
    pub const ALL: [GameSet; 5] = [
        GameSet::Base,
        GameSet::Daybreak,
        GameSet::Vampire,
        GameSet::Alien,
        GameSet::SuperVillains,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            GameSet::Base => "base",
            GameSet::Daybreak => "daybreak",
            GameSet::Vampire => "vampire",
            GameSet::Alien => "alien",
            GameSet::SuperVillains => "villains",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GameSet::Base => "Base Set",
            GameSet::Daybreak => "Daybreak",
            GameSet::Vampire => "Vampire",
            GameSet::Alien => "Alien",
            GameSet::SuperVillains => "Super Villains",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let norm = raw
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_");
        match norm.as_str() {
            "base" | "classic" | "onuw" => Some(GameSet::Base),
            "daybreak" | "db" => Some(GameSet::Daybreak),
            "vampire" | "vamp" | "vampires" => Some(GameSet::Vampire),
            "alien" | "aliens" => Some(GameSet::Alien),
            "super_villains" | "supervillains" | "villains" | "sv" => Some(GameSet::SuperVillains),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Team {
    /// Villagers, Heroes, and village-aligned specialists.
    Village,
    /// Werewolves and Minion.
    Werewolf,
    /// Vampires (`Vampire`, `The Master`, `Count`), Renfield, and `Mark::Vampire` thralls.
    Vampire,
    /// Aliens (`Alien`, solo `Groob`/`Zerb`).
    Alien,
    /// Super Villains (`Henchman`, `Temptress`, `Dr. Peeker`, `Rapscallion`).
    SuperVillain,
    /// Solo / custom win condition (`Tanner`, `Assassin`, `Groob`, `Zerb`, `Blob`, `Mortician`, `Mad Scientist`, `Intern`).
    Independent,
}

impl Team {
    /// All four hostile monster factions used for *Epic Battle* detection and Tanner-style team blocks.
    pub const MONSTER_TEAMS: &'static [Team] = &[
        Team::Werewolf,
        Team::Vampire,
        Team::Alien,
        Team::SuperVillain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Team::Village => "Village",
            Team::Werewolf => "Werewolf",
            Team::Vampire => "Vampire",
            Team::Alien => "Alien",
            Team::SuperVillain => "Super Villain",
            Team::Independent => "Independent",
        }
    }

    pub fn win_label(self) -> &'static str {
        match self {
            Team::Village => "Village",
            Team::Werewolf => "Werewolf Team",
            Team::Vampire => "Vampire Team",
            Team::Alien => "Alien Team",
            Team::SuperVillain => "Super Villain Team",
            Team::Independent => "Independent",
        }
    }

    pub fn style_text(self, s: &'static str) -> Text {
        match self {
            Team::Village => s.bold().cyan().into(),
            Team::Werewolf | Team::SuperVillain => s.bold().red().into(),
            Team::Vampire => s.bold().magenta().into(),
            Team::Alien => s.bold().green().into(),
            Team::Independent => s.bold().orange().into(),
        }
    }

    pub fn styled_label(self) -> Text {
        self.style_text(self.label())
    }
}

/// Passive monster-group identity registered during Dusk/Night (distinct from endgame [`Team`]).
///
/// For example:
/// - `Dream Wolf` has `Some(NightFaction::Werewolf)` (seen by Wolves & `Minion`) while sleeping (`steps() == &[]`).
/// - `Synthetic Alien` has `Team::Independent` at Voting, but `Some(NightFaction::Alien)` during Night.
/// - `Minion` has `Team::Werewolf` at Voting, but `None` during Night.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NightFaction {
    Werewolf,
    Vampire,
    Alien,
    SuperVillain,
}

impl NightFaction {
    pub fn team(self) -> Team {
        match self {
            Self::Werewolf => Team::Werewolf,
            Self::Vampire => Team::Vampire,
            Self::Alien => Team::Alien,
            Self::SuperVillain => Team::SuperVillain,
        }
    }

    pub fn step_id(self) -> &'static str {
        match self {
            Self::Werewolf => "pack:werewolf",
            Self::Vampire => "pack:vampire",
            Self::Alien => "pack:alien",
            Self::SuperVillain => "pack:super_villain",
        }
    }

    pub fn callout_name(self) -> &'static str {
        match self {
            Self::Werewolf => "Werewolves",
            Self::Vampire => "Vampires",
            Self::Alien => "Aliens",
            Self::SuperVillain => "Super Villains",
        }
    }

    pub fn styled_callout(self) -> Text {
        self.team().style_text(self.callout_name())
    }
}

/// Static card identity, team alignment, and deck metadata returned by [`Role::info`].
#[derive(Clone, Copy, Debug)]
pub struct RoleInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub game: GameSet,
    pub team: Team,
    pub threat_team: Option<Team>,
    pub night_faction: Option<NightFaction>,
    pub summary: &'static str,
    pub aliases: &'static [&'static str],
    pub is_minion: bool,
    pub places_marks: bool,
    pub requires_pair_in_deck: bool,
    pub anarchy_bundle: Option<&'static str>,
    pub extra_non_village_labels: &'static [&'static str],
}

impl RoleInfo {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        game: GameSet,
        team: Team,
        summary: &'static str,
    ) -> Self {
        let threat_team = match team {
            Team::Werewolf | Team::Vampire | Team::Alien | Team::SuperVillain => Some(team),
            Team::Village | Team::Independent => None,
        };
        Self {
            id,
            name,
            game,
            team,
            threat_team,
            night_faction: None,
            summary,
            aliases: &[],
            is_minion: false,
            places_marks: false,
            requires_pair_in_deck: false,
            anarchy_bundle: None,
            extra_non_village_labels: &[],
        }
    }

    pub const fn aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }

    pub const fn night_faction(mut self, faction: NightFaction) -> Self {
        self.night_faction = Some(faction);
        self
    }

    pub const fn places_marks(mut self) -> Self {
        self.places_marks = true;
        self
    }

    pub const fn minion(mut self) -> Self {
        self.is_minion = true;
        self.threat_team = None;
        self
    }

    pub const fn threat_team(mut self, team: Team) -> Self {
        self.threat_team = Some(team);
        self
    }

    pub const fn paired(mut self) -> Self {
        self.requires_pair_in_deck = true;
        self
    }

    pub const fn anarchy_bundle(mut self, group: &'static str) -> Self {
        self.anarchy_bundle = Some(group);
        self
    }

    pub const fn non_village_labels(mut self, labels: &'static [&'static str]) -> Self {
        self.extra_non_village_labels = labels;
        self
    }

    pub fn styled_name(self) -> Text {
        self.team.style_text(self.name)
    }
}

// ============================================================================
// Dusk / Night Schedule (`WakeStep`) & Deferred Resolution Closures
// ============================================================================

/// Phase in the Dusk/Night schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Vampire-expansion Dusk phase (`#-9..#-1B`, followed at `00` by Mark inspection & `Lovers`).
    Dusk,
    /// Standard Night phase (`#0..#13A`).
    Night,
}

/// How `Doppelgänger` executes a copied role's step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyTiming {
    /// Copied action runs in the `DoppelgangerImmediate` window right after `Doppelgänger` (`#1` / `#-7`).
    Immediate,
    /// Copied action runs in the role's scheduled Dusk/Night tier right after initial holders (`X-A`).
    OnRoleTurn,
}

/// Deferred resolution closure returned by a solo role's DM prompt, invoked at the fixed-clock
/// tier deadline with `(actor_label, &mut BoardMut, &mut ResolveCtx)`.
pub struct SeatEffect(
    Box<dyn FnOnce(&str, &mut BoardMut<'_>, &mut ResolveCtx<'_>) + Send>,
);

impl SeatEffect {
    pub fn new(
        f: impl FnOnce(&str, &mut BoardMut<'_>, &mut ResolveCtx<'_>) + Send + 'static,
    ) -> Self {
        Self(Box::new(f))
    }

    pub fn noop() -> Self {
        Self::new(|_, _, _| {})
    }

    pub(crate) fn apply(
        self,
        actor_label: &str,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        (self.0)(actor_label, board, ctx);
    }
}

/// Deferred resolution closure returned by a group/pack role's prompt, invoked at the fixed-clock
/// tier deadline with `(&mut BoardMut, &mut ResolveCtx)`.
pub struct GroupEffect(Box<dyn FnOnce(&mut BoardMut<'_>, &mut ResolveCtx<'_>) + Send>);

impl GroupEffect {
    pub fn new(f: impl FnOnce(&mut BoardMut<'_>, &mut ResolveCtx<'_>) + Send + 'static) -> Self {
        Self(Box::new(f))
    }

    pub fn noop() -> Self {
        Self::new(|_, _| {})
    }

    pub(crate) fn apply(self, board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        (self.0)(board, ctx);
    }
}

pub type SoloPromptFn = for<'a> fn(usize, PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>>;
pub type GroupPromptFn =
    for<'a> fn(SeatSet, GroupPromptCtx<'a>) -> BoxFuture<'a, Result<GroupEffect>>;
pub type PassiveWakeFn = fn(usize, &str, &mut BoardMut<'_>, &mut ResolveCtx<'_>);

#[derive(Clone, Copy)]
pub(crate) enum StepKind {
    Solo(SoloPromptFn),
    Group(GroupPromptFn),
    Passive(PassiveWakeFn),
}

#[derive(Clone, Copy)]
enum StepTarget {
    Role(&'static dyn Role),
    Pack(NightFaction),
    Pair {
        id: &'static str,
        callout: &'static str,
        team: Team,
        faction: Option<NightFaction>,
    },
}

/// Declarative Dusk/Night schedule entry contributed by [`Role::steps`].
///
/// Defaults across all constructors:
/// - `eyes_open = true` (override with `.eyes_closed()` for eyes-closed roles like `Cow` and `Evilometer`).
/// - `prompt_reads = VISIBLE_TABLE` for interactive steps (extend with `.reads(...)` when reading other zones mid-prompt).
/// - `resolve_writes = NONE` (set with `.writes(...)` when mutating board state at resolution time).
/// - `copy_timing = Immediate` for [`WakeStep::solo`] (override with `.copy_on_turn()` when `Doppelgänger` waits for the role's scheduled turn), `OnRoleTurn` for `group`/`passive`, `None` for `pack`/`doppelganger`.
#[derive(Clone, Copy)]
pub struct WakeStep {
    target: StepTarget,
    pub phase: Phase,
    pub order: u16,
    pub eyes_open: bool,
    pub short_budget: bool,
    pub is_doppelganger: bool,
    pub run_when_unheld: bool,
    pub copy_timing: Option<CopyTiming>,
    pub prompt_reads: BoardAccess,
    pub resolve_writes: BoardAccess,
    pub(crate) kind: StepKind,
}

impl fmt::Debug for WakeStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WakeStep({}, {:?} #{})",
            self.callout_name(),
            self.phase,
            self.order
        )
    }
}

impl WakeStep {
    /// Interactive per-seat DM prompt (`eyes_open = true`, `copy_timing = Immediate` by default).
    pub const fn solo(owner: &'static dyn Role, order: u16, f: SoloPromptFn) -> Self {
        Self {
            target: StepTarget::Role(owner),
            phase: Phase::Night,
            order,
            eyes_open: true,
            short_budget: false,
            is_doppelganger: false,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::Immediate),
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            kind: StepKind::Solo(f),
        }
    }

    /// `Doppelgänger`: wakes at `#-7` (`order 10`) when Dusk roles are in play, or `#1` (`order 105`) otherwise.
    pub const fn doppelganger(owner: &'static dyn Role, f: SoloPromptFn) -> Self {
        Self::solo(owner, 105, f)
            .short_budget()
            .writes(BoardAccess::PLAYER_BECAME | BoardAccess::WAKING_ROLES)
            .mark_doppelganger()
    }

    const fn mark_doppelganger(mut self) -> Self {
        self.is_doppelganger = true;
        self.copy_timing = None;
        self
    }

    /// Role-owned group wake (`Mason`, `Minion`, `Leader`, `Intern`, `Groob`, `Zerb`, `Apprentice Assassin`).
    pub const fn group(owner: &'static dyn Role, order: u16, f: GroupPromptFn) -> Self {
        Self {
            target: StepTarget::Role(owner),
            phase: Phase::Night,
            order,
            eyes_open: true,
            short_budget: false,
            is_doppelganger: false,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::OnRoleTurn),
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            kind: StepKind::Group(f),
        }
    }

    /// Shared monster-pack wake (`Werewolves`, `Vampires`, `Aliens`, `Super Villains`).
    pub const fn pack(faction: NightFaction, order: u16, f: GroupPromptFn) -> Self {
        Self {
            target: StepTarget::Pack(faction),
            phase: Phase::Night,
            order,
            eyes_open: true,
            short_budget: false,
            is_doppelganger: false,
            run_when_unheld: false,
            copy_timing: None,
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            kind: StepKind::Group(f),
        }
    }

    /// Shared pair/rival group wake (`Groob & Zerb` `#1D`).
    pub const fn pair(
        id: &'static str,
        callout: &'static str,
        team: Team,
        faction: Option<NightFaction>,
        order: u16,
        f: GroupPromptFn,
    ) -> Self {
        Self {
            target: StepTarget::Pair {
                id,
                callout,
                team,
                faction,
            },
            phase: Phase::Night,
            order,
            eyes_open: true,
            short_budget: false,
            is_doppelganger: false,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::OnRoleTurn),
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            kind: StepKind::Group(f),
        }
    }

    /// Passive resolution-time wake (`eyes_open = true` by default; chain `.eyes_closed()` for `Cow`/`Evilometer`).
    pub const fn passive(owner: &'static dyn Role, order: u16, f: PassiveWakeFn) -> Self {
        Self {
            target: StepTarget::Role(owner),
            phase: Phase::Night,
            order,
            eyes_open: true,
            short_budget: false,
            is_doppelganger: false,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::OnRoleTurn),
            prompt_reads: BoardAccess(BoardAccess::NONE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            kind: StepKind::Passive(f),
        }
    }

    pub const fn run_when_unheld(mut self) -> Self {
        assert!(
            matches!(self.kind, StepKind::Group(_)),
            "only Group steps support .run_when_unheld()"
        );
        self.run_when_unheld = true;
        self
    }

    pub const fn reads(mut self, mask: u16) -> Self {
        self.prompt_reads.0 |= mask;
        self
    }

    pub const fn writes(mut self, mask: u16) -> Self {
        self.resolve_writes.0 |= mask;
        self
    }

    pub const fn copy_on_turn(mut self) -> Self {
        assert!(
            matches!(self.kind, StepKind::Solo(_)),
            "only Solo steps support .copy_on_turn() (group and passive steps already wake on turn)"
        );
        self.copy_timing = Some(CopyTiming::OnRoleTurn);
        self
    }

    pub const fn eyes_closed(mut self) -> Self {
        self.eyes_open = false;
        self.prompt_reads.0 &= !BoardAccess::VISIBLE_TABLE;
        self
    }

    pub const fn dusk(mut self) -> Self {
        self.phase = Phase::Dusk;
        self
    }

    pub const fn short_budget(mut self) -> Self {
        self.short_budget = true;
        self
    }

    pub const fn needs_dm_prompt(&self) -> bool {
        !matches!(self.kind, StepKind::Passive(_))
    }

    pub fn id(&self) -> &'static str {
        match self.target {
            StepTarget::Role(r) => r.id(),
            StepTarget::Pack(f) => f.step_id(),
            StepTarget::Pair { id, .. } => id,
        }
    }

    pub fn callout_name(&self) -> &'static str {
        match self.target {
            StepTarget::Role(r) => r.name(),
            StepTarget::Pack(f) => f.callout_name(),
            StepTarget::Pair { callout, .. } => callout,
        }
    }

    pub fn styled_callout(&self) -> Text {
        match self.target {
            StepTarget::Role(r) => r.styled_name(),
            StepTarget::Pack(f) => f.styled_callout(),
            StepTarget::Pair { callout, team, .. } => team.style_text(callout),
        }
    }

    pub fn pack_faction(&self) -> Option<NightFaction> {
        match self.target {
            StepTarget::Pack(f) => Some(f),
            StepTarget::Role(_) | StepTarget::Pair { .. } => None,
        }
    }

    pub fn owner_night_faction(&self) -> Option<NightFaction> {
        match self.target {
            StepTarget::Role(r) => r.night_faction(),
            StepTarget::Pack(_) => None,
            StepTarget::Pair { faction, .. } => faction,
        }
    }

    /// Effective read mask for scheduler conflict detection & runtime `PromptCtx` checks:
    /// every interactive step implicitly depends on `WAKING_ROLES`, plus `VISIBLE_TABLE` when `eyes_open`.
    pub fn effective_prompt_reads(&self) -> u16 {
        if !self.needs_dm_prompt() {
            return BoardAccess::NONE;
        }
        let vis = if self.eyes_open {
            BoardAccess::VISIBLE_TABLE
        } else {
            BoardAccess::NONE
        };
        self.prompt_reads.0 | BoardAccess::WAKING_ROLES | vis
    }
}

// ============================================================================
// Ephemeral Night Waker State & Hook Contexts
// ============================================================================

/// Ephemeral per-seat Night waking state (dropped when [`crate::night::run_night_phase`] finishes
/// so [`BoardCards`] carries only physical tabletop state into Day & Voting).
pub(crate) struct NightWakers {
    waking_roles: Vec<&'static dyn Role>,
    copied_from: Vec<Option<usize>>,
    pub acted_immediately: SeatSet,
    pub feared: SeatSet,
    pub wake_log_recipients: SeatSet,
    pub wake_log: Vec<(Phase, String)>,
}

impl NightWakers {
    pub fn new(roster: &Roster) -> Self {
        Self {
            waking_roles: roster.seats.iter().map(|s| s.initial_role).collect(),
            copied_from: vec![None; roster.len()],
            acted_immediately: SeatSet::EMPTY,
            feared: SeatSet::EMPTY,
            wake_log_recipients: SeatSet::EMPTY,
            wake_log: Vec::new(),
        }
    }

    pub fn set_role(&mut self, seat: usize, role: &'static dyn Role) {
        self.waking_roles[seat] = role;
    }

    pub fn roles(&self) -> &[&'static dyn Role] {
        &self.waking_roles
    }

    pub fn role(&self, seat: usize) -> &'static dyn Role {
        self.waking_roles[seat]
    }

    pub fn copied_from(&self, seat: usize) -> Option<usize> {
        self.copied_from[seat]
    }

    pub fn doppelganger_copied_seats(&self) -> SeatSet {
        self.copied_from
            .iter()
            .enumerate()
            .filter_map(|(i, src)| src.map(|_| i))
            .collect()
    }

    pub fn actor_label(&self, roster: &Roster, seat: usize) -> String {
        let initial = roster.seats[seat].initial_role;
        let waking = self.waking_roles[seat];
        if waking.id() == initial.id() {
            initial.name().to_owned()
        } else {
            format!("{}-{}", initial.name(), waking.name())
        }
    }

    pub fn copy_player(&mut self, actor: usize, target: usize, face_role: &'static dyn Role) {
        self.copied_from[actor] = Some(target);
        self.waking_roles[actor] = face_role;
    }

    pub fn copy_center(&mut self, actor: usize, copied: &'static dyn Role) {
        if !copied.is_doppelganger() {
            self.waking_roles[actor] = copied;
        }
    }

    pub fn seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.waking_roles
            .iter()
            .enumerate()
            .filter_map(|(i, r)| (r.night_faction() == Some(faction)).then_some(i))
            .collect()
    }

    pub fn waking_seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.seats_with_faction(faction) - self.feared
    }

    pub fn seats_with_role(&self, role: &'static dyn Role) -> SeatSet {
        let target_id = role.id();
        self.waking_roles
            .iter()
            .enumerate()
            .filter_map(|(i, r)| (r.id() == target_id).then_some(i))
            .collect()
    }

    pub fn waking_seats_with_role(&self, role: &'static dyn Role) -> SeatSet {
        self.seats_with_role(role) - self.feared
    }
}

pub struct PromptCtx<'a> {
    step_name: &'static str,
    reads: u16,
    seat: usize,
    eyes_open: bool,
    is_immediate_copy: bool,
    dm: Option<ExclusiveDm>,
    pub roster: &'a Roster,
    pub ask_cfg: &'a AskConfig,
    pub deadline: Instant,
    wakers: &'a NightWakers,
    board: PromptBoard<'a>,
    saw_visible_table: AtomicBool,
}

/// `Copy` read-only snapshot view extracted from [`PromptCtx::view`] for capture inside
/// `ctx.ask(prompt, move |m| ...)` closures while `ctx` is mutably borrowed.
#[derive(Clone, Copy)]
pub struct PromptView<'a> {
    step_name: &'static str,
    reads: u16,
    pub roster: &'a Roster,
    board: PromptBoard<'a>,
}

impl<'a> PromptView<'a> {
    #[inline]
    fn check_read(self, mask: u16, zone: &str) {
        assert!(
            (self.reads & mask) == mask,
            "{}: undeclared prompt read of {zone} — add .reads(BoardAccess::{zone}) to WakeStep",
            self.step_name
        );
    }

    pub fn resolve_nick(self, nick: &str) -> Option<usize> {
        self.roster.resolve_nick(nick)
    }

    pub fn is_shielded(self, seat: usize) -> bool {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.board.raw().is_shielded(seat)
    }

    pub fn shielded_seats(self) -> SeatSet {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.board.raw().shielded_seats()
    }

    pub fn has_artifact(self, seat: usize) -> bool {
        self.check_read(BoardAccess::ARTIFACTS, "ARTIFACTS");
        self.board.raw().artifact(seat).is_some()
    }

    pub fn is_face_up(self, seat: usize) -> bool {
        self.check_read(BoardAccess::FACE_UP, "FACE_UP");
        self.board.raw().is_player_face_up(seat)
    }
}

impl<'a> PromptCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        step_name: &'static str,
        reads: u16,
        seat: usize,
        eyes_open: bool,
        dm: Option<ExclusiveDm>,
        roster: &'a Roster,
        ask_cfg: &'a AskConfig,
        deadline: Instant,
        wakers: &'a NightWakers,
        board: PromptBoard<'a>,
    ) -> Self {
        Self {
            step_name,
            reads,
            seat,
            eyes_open,
            is_immediate_copy: false,
            dm,
            roster,
            ask_cfg,
            deadline,
            wakers,
            board,
            saw_visible_table: AtomicBool::new(false),
        }
    }

    pub(crate) fn as_immediate_copy(mut self) -> Self {
        self.is_immediate_copy = true;
        self
    }

    #[inline]
    fn check_read(&self, mask: u16, zone: &str) {
        assert!(
            (self.reads & mask) == mask,
            "{}: undeclared prompt read of {zone} — add .reads(BoardAccess::{zone}) to WakeStep",
            self.step_name
        );
    }

    pub fn view(&self) -> PromptView<'a> {
        PromptView {
            step_name: self.step_name,
            reads: self.reads,
            roster: self.roster,
            board: self.board,
        }
    }

    fn maybe_emit_table_summary(&self) {
        if self.eyes_open && !self.saw_visible_table.swap(true, Ordering::Relaxed) {
            if let Some(dm) = &self.dm {
                if let Some(summary) = self.board.raw().visible_table_summary(self.roster) {
                    let _ = dm.say(summary);
                }
            }
        }
    }

    /// Sends `prompt` in DM using the tier's `AskConfig`. If the player already disconnected
    /// before the tier started, immediately returns `Ok(Answer::Left { .. })` without sending traffic.
    pub async fn ask<F, R>(
        &mut self,
        prompt: impl Into<Text>,
        matcher: F,
    ) -> Result<Answer<R::Output>>
    where
        F: FnMut(&ChatMessage) -> R + Send,
        R: IntoAskMatch,
        R::Output: Send,
    {
        self.maybe_emit_table_summary();
        match &mut self.dm {
            Some(dm) => dm.ask(prompt, matcher).config(self.ask_cfg).await,
            None => Ok(Answer::Left {
                nick: self.nick(self.seat),
                reason: None,
            }),
        }
    }

    pub fn say(&self, text: impl Into<Text>) {
        self.maybe_emit_table_summary();
        if let Some(dm) = &self.dm {
            let _ = dm.say(text);
        }
    }

    pub fn nick(&self, seat: usize) -> String {
        self.roster.seats[seat].user.nick()
    }

    pub fn is_shielded(&self, seat: usize) -> bool {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.board.raw().is_shielded(seat)
    }

    pub fn shielded_seats(&self) -> SeatSet {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.board.raw().shielded_seats()
    }

    pub fn has_artifact(&self, seat: usize) -> bool {
        self.check_read(BoardAccess::ARTIFACTS, "ARTIFACTS");
        self.board.raw().artifact(seat).is_some()
    }

    /// Formats all seats with inline visible tabletop markers (`🛡`, `[🔦 Role]`, `🏺`).
    pub fn format_visible_players(&self) -> String {
        self.check_read(BoardAccess::VISIBLE_TABLE, "VISIBLE_TABLE");
        self.saw_visible_table.store(true, Ordering::Relaxed);
        self.board.raw().format_visible_players(self.roster)
    }

    /// Formats `seats` with inline visible tabletop markers (`🛡`, `[🔦 Role]`, `🏺`).
    pub fn format_visible_seats(&self, seats: SeatSet) -> String {
        self.check_read(BoardAccess::VISIBLE_TABLE, "VISIBLE_TABLE");
        self.board.raw().format_visible_seats(self.roster, seats)
    }

    /// Reads `players[seat]` mid-prompt (`Paranormal Investigator`).
    pub fn peek_player_card(&self, seat: usize) -> Card {
        self.check_read(
            BoardAccess::PLAYER_FACES | BoardAccess::SHIELDS,
            "PLAYER_FACES | SHIELDS",
        );
        self.board.raw().player_card(seat)
    }

    /// Reads `center[slot]` mid-prompt (`Witch`).
    pub fn peek_center_card(&self, slot: usize) -> Card {
        self.check_read(BoardAccess::CENTER_FACES, "CENTER_FACES");
        self.board.raw().center_card(slot)
    }

    /// Reads `marks[seat]` mid-prompt (`Count`).
    pub fn mark_at(&self, seat: usize) -> Mark {
        self.check_read(BoardAccess::MARKS, "MARKS");
        self.board.raw().mark(seat)
    }

    /// Reads typed blackboard state `K` mid-prompt (`Psychic` reading `OracleOmen`).
    pub fn get_key<K: BoardKey>(&self) -> K {
        self.check_read(K::ACCESS, "BoardKey::ACCESS");
        self.board.raw().get_key::<K>()
    }

    /// All seats holding `faction` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
    pub fn seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.seats_with_faction(faction)
    }

    /// Waking seats holding `faction` (excluding seats suppressed by `Mark::Fear`).
    pub fn waking_seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.waking_seats_with_faction(faction)
    }

    /// Packmates known to `self.seat` at the time of this prompt:
    /// - After the pack huddle (`OnRoleTurn`), all waking members of `faction`.
    /// - Before the pack huddle (`DoppelgangerImmediate` at `#1`), only `self.seat` and the player
    ///   card she just copied (so unrevealed Werewolves are not leaked before `#2`).
    pub fn known_pack_mates(&self, faction: NightFaction) -> SeatSet {
        if self.is_immediate_copy {
            let mut known = SeatSet::single(self.seat);
            if let Some(src) = self.wakers.copied_from(self.seat) {
                known.insert(src);
            }
            known
        } else {
            self.wakers.waking_seats_with_faction(faction)
        }
    }
}

pub struct GroupPromptCtx<'a> {
    step_name: &'static str,
    reads: u16,
    eyes_open: bool,
    pub bot: &'a Bot,
    pub roster: &'a Roster,
    pub ask_cfg: &'a AskConfig,
    pub deadline: Instant,
    pub waking_roles: &'a [&'static dyn Role],
    wakers: &'a NightWakers,
    board: PromptBoard<'a>,
    notified_table_seats: AtomicU64,
}

impl<'a> GroupPromptCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        step_name: &'static str,
        reads: u16,
        eyes_open: bool,
        bot: &'a Bot,
        roster: &'a Roster,
        ask_cfg: &'a AskConfig,
        deadline: Instant,
        wakers: &'a NightWakers,
        board: PromptBoard<'a>,
    ) -> Self {
        Self {
            step_name,
            reads,
            eyes_open,
            bot,
            roster,
            ask_cfg,
            deadline,
            waking_roles: wakers.roles(),
            wakers,
            board,
            notified_table_seats: AtomicU64::new(0),
        }
    }

    #[inline]
    fn check_read(&self, mask: u16, zone: &str) {
        assert!(
            (self.reads & mask) == mask,
            "{}: undeclared group prompt read of {zone} — add .reads(BoardAccess::{zone}) to WakeStep",
            self.step_name
        );
    }

    fn maybe_emit_table_summary_to(&self, seat: usize) {
        if !self.eyes_open || seat >= SeatSet::MAX_SEATS {
            return;
        }
        let bit = 1u64 << seat;
        if (self.notified_table_seats.fetch_or(bit, Ordering::Relaxed) & bit) == 0 {
            if let Some(summary) = self.board.raw().visible_table_summary(self.roster) {
                let _ = self.roster.seats[seat].user.say(summary);
            }
        }
    }

    pub async fn ask<F, R>(
        &self,
        seat: usize,
        prompt: impl Into<Text>,
        matcher: F,
    ) -> Result<Answer<R::Output>>
    where
        F: FnMut(&ChatMessage) -> R + Send,
        R: IntoAskMatch,
        R::Output: Send,
    {
        self.maybe_emit_table_summary_to(seat);
        self.roster.seats[seat]
            .user
            .ask(prompt, matcher)
            .config(self.ask_cfg)
            .await
    }

    pub fn dm(&self, seat: usize, text: impl Into<Text>) {
        self.maybe_emit_table_summary_to(seat);
        let _ = self.roster.seats[seat].user.say(text);
    }

    /// Opens a drop-out-tolerant DM [`Huddle`] across `seats` using [`Bot::huddle_available`].
    pub async fn huddle(&self, name: impl Into<Text>, seats: SeatSet) -> Result<Huddle> {
        for s in seats {
            self.maybe_emit_table_summary_to(s);
        }
        let users: Vec<User> = seats
            .into_iter()
            .map(|s| self.roster.seats[s].user.clone())
            .collect();
        self.bot.huddle_available(name, &users).await
    }

    /// Opens a drop-out-tolerant DM [`Huddle`] across `seats` and keeps its pump-dispatched
    /// relay active until the tier deadline.
    pub async fn relay_huddle(&self, name: impl Into<Text>, seats: SeatSet) -> Result<()> {
        let _huddle = self.huddle(name, seats).await?;
        tokio::time::sleep_until(self.deadline).await;
        Ok(())
    }

    pub fn nick(&self, seat: usize) -> String {
        self.roster.seats[seat].user.nick()
    }

    pub fn is_shielded(&self, seat: usize) -> bool {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.board.raw().is_shielded(seat)
    }

    pub fn shielded_seats(&self) -> SeatSet {
        self.check_read(BoardAccess::SHIELDS, "SHIELDS");
        self.board.raw().shielded_seats()
    }

    pub fn format_visible_seats(&self, seats: SeatSet) -> String {
        self.check_read(BoardAccess::VISIBLE_TABLE, "VISIBLE_TABLE");
        self.board.raw().format_visible_seats(self.roster, seats)
    }

    /// All seats holding `faction` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
    pub fn seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.seats_with_faction(faction)
    }

    /// Waking seats holding `faction` (excluding seats suppressed by `Mark::Fear`).
    pub fn waking_seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.waking_seats_with_faction(faction)
    }

    /// All seats holding `role` (initial role or copied by `Copycat` / `Doppelgänger`).
    pub fn seats_with_role(&self, role: &'static dyn Role) -> SeatSet {
        self.wakers.seats_with_role(role)
    }

    /// Waking seats holding `role` (excluding seats suppressed by `Mark::Fear`).
    pub fn waking_seats_with_role(&self, role: &'static dyn Role) -> SeatSet {
        self.wakers.waking_seats_with_role(role)
    }

    /// Seat that `seat` copied via `Doppelgänger` (`None` for non-Doppelgänger seats).
    pub fn copied_from(&self, seat: usize) -> Option<usize> {
        self.wakers.copied_from(seat)
    }

    /// Reads typed blackboard state `K` mid-prompt (`Alien` reading `Oracle` directives, etc.).
    pub fn get_key<K: BoardKey>(&self) -> K {
        self.check_read(K::ACCESS, "BoardKey::ACCESS");
        self.board.raw().get_key::<K>()
    }
}

pub struct ResolveCtx<'a> {
    pub roster: &'a Roster,
    pub chronicle: &'a mut Vec<Text>,
    pub(crate) room: &'a Room,
    pub(crate) wakers: &'a mut NightWakers,
}

impl<'a> ResolveCtx<'a> {
    pub fn record(&mut self, entry: impl Into<Text>) {
        self.chronicle.push(entry.into());
    }

    pub fn announce(&self, text: impl Into<Text>) {
        let _ = self.room.say(text);
    }

    pub fn nick(&self, seat: usize) -> String {
        self.roster.seats[seat].user.nick()
    }

    pub fn dm(&self, seat: usize, text: impl Into<Text>) {
        let _ = self.roster.seats[seat].user.say(text);
    }

    pub fn waking_role(&self, seat: usize) -> &'static dyn Role {
        self.wakers.role(seat)
    }

    pub fn grant_wake_log(&mut self, seat: usize) {
        self.wakers.wake_log_recipients.insert(seat);
    }
}

pub struct DeathCtx<'a> {
    pub roster: &'a Roster,
    pub board: &'a BoardCards,
    pub votes: &'a [Option<usize>],
}

pub struct VoteProtection {
    pub protected_seat: usize,
    /// True if this protection also blocks `Hunter`-style death shots (`Bodyguard`), false for vote-only immunity (`Prince`).
    pub blocks_hunter_shot: bool,
    pub announcement: Text,
}

pub enum DeathEffect {
    /// Immediately kills another seat (`Hunter`, `Mark::Love`), cascading in the Voting fixpoint loop.
    Kill {
        seat: usize,
        respects_protection: bool,
        announcement: Text,
    },
}

pub struct VictoryCtx<'a> {
    pub roster: &'a Roster,
    pub board: &'a BoardCards,
    pub votes: &'a [Option<usize>],
    pub dead_seats: SeatSet,
}

impl<'a> VictoryCtx<'a> {
    pub fn is_dead(&self, seat: usize) -> bool {
        self.dead_seats.contains(seat)
    }

    /// Seats currently holding `role` un-overridden by role-replacing `Artifact`s or `Mark::Vampire`.
    pub fn active_holders(&self, role: &'static dyn Role) -> SeatSet {
        self.board.active_holders(role)
    }

    /// Seats whose card is `role` un-overridden by role-replacing `Artifact`s (including `Mark::Vampire` holders).
    pub fn role_card_holders(&self, role: &'static dyn Role) -> SeatSet {
        self.board.role_card_holders(role)
    }
}

/// Which teams are prevented from winning when an `Independent` role wins (`Tanner` vs `Mad Scientist`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TeamBlock {
    #[default]
    None,
    MonsterTeams,
    AllTeams,
}

impl TeamBlock {
    pub fn blocks(self, team: Team) -> bool {
        match self {
            Self::None => false,
            Self::MonsterTeams => Team::MONSTER_TEAMS.contains(&team),
            Self::AllTeams => team != Team::Independent,
        }
    }
}

#[derive(Clone, Debug)]
pub enum SpecialVictory {
    /// Personal victory for an `Independent` role or artifact holder, optionally blocking teams from winning.
    Independent(TeamBlock),
    /// Overrides normal team victory rules so `team` wins (`Leader` unanimous Alien point).
    ForceTeam { team: Team, announcement: Text },
    /// Global hunt override (`Oracle` `#7` wrong guess): holders win solo iff surviving;
    /// every other player wins iff at least one holder dies.
    Hunted,
}

impl SpecialVictory {
    pub const fn independent() -> Self {
        Self::Independent(TeamBlock::None)
    }

    pub const fn independent_blocking_monsters() -> Self {
        Self::Independent(TeamBlock::MonsterTeams)
    }

    pub const fn independent_sole_winner() -> Self {
        Self::Independent(TeamBlock::AllTeams)
    }

    pub fn force_team(team: Team, announcement: impl Into<Text>) -> Self {
        Self::ForceTeam {
            team,
            announcement: announcement.into(),
        }
    }
}

/// Final team and monster-threat status of a seat after folding `Role < Mark < Artifact`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectiveAlignment {
    pub team: Team,
    pub threat_team: Option<Team>,
}

pub type WinnerMap = HashMap<usize, &'static str>;

pub struct WinnerFilterCtx<'a> {
    pub roster: &'a Roster,
    pub board: &'a BoardCards,
    pub votes: &'a [Option<usize>],
    pub dead_seats: SeatSet,
    pub team_block: TeamBlock,
    pub casemap: CaseMap,
    pub day_channel_log: &'a [ChatMessage],
    pub announcements: &'a mut Vec<Text>,
}

/// Unified post-vote alignment, immunity, death-chain, victory, and win-forfeit hook trait
/// implemented across [`Role`], [`Mark`], and [`crate::artifacts::Artifact`].
pub trait EndgameModifier: Send + Sync {
    fn modifier_name(&self) -> &'static str;

    fn adjust_alignment(
        &self,
        _seat: usize,
        _board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur
    }

    fn vote_protection(&self, _seat: usize, _ctx: &DeathCtx<'_>) -> Option<VoteProtection> {
        None
    }

    fn on_death(&self, _dead_seat: usize, _ctx: &DeathCtx<'_>) -> Vec<DeathEffect> {
        Vec::new()
    }

    fn victory_claim(&self, _seat: usize, _ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        None
    }

    fn override_win(
        &self,
        _holder: usize,
        _ctx: &WinnerFilterCtx<'_>,
        _winners: &mut WinnerMap,
    ) {
    }

    fn apply_forfeits(
        &self,
        _holder: usize,
        _ctx: &mut WinnerFilterCtx<'_>,
        _winners: &mut WinnerMap,
    ) {
    }
}

// ============================================================================
// Object-Safe `Role` Trait
// ============================================================================

/// Object-safe role trait implemented by all 59 cards across Base + 4 Expansions.
pub trait Role: Send + Sync + 'static {
    fn info(&self) -> RoleInfo;

    fn summary_for_deck(&self, _roster: &Roster) -> String {
        self.info().summary.to_owned()
    }

    fn steps(&self) -> &'static [WakeStep] {
        &[]
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        _board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur
    }

    fn vote_protection(&self, _seat: usize, _ctx: &DeathCtx<'_>) -> Option<VoteProtection> {
        None
    }

    fn on_death(&self, _dead_seat: usize, _ctx: &DeathCtx<'_>) -> Option<DeathEffect> {
        None
    }

    fn special_victory(&self, _seat: usize, _ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        None
    }

    fn apply_forfeits(
        &self,
        _holder: usize,
        _ctx: &mut WinnerFilterCtx<'_>,
        _winners: &mut WinnerMap,
    ) {
    }
}

impl EndgameModifier for &'static dyn Role {
    fn modifier_name(&self) -> &'static str {
        self.name()
    }

    fn adjust_alignment(
        &self,
        seat: usize,
        board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        (**self).adjust_alignment(seat, board, cur)
    }

    fn vote_protection(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteProtection> {
        (**self).vote_protection(seat, ctx)
    }

    fn on_death(&self, dead_seat: usize, ctx: &DeathCtx<'_>) -> Vec<DeathEffect> {
        (**self).on_death(dead_seat, ctx).into_iter().collect()
    }

    fn victory_claim(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        match (**self).special_victory(seat, ctx)? {
            SpecialVictory::ForceTeam { team, announcement } => {
                (ctx.board.effective_team(seat) == self.team())
                    .then_some(SpecialVictory::ForceTeam { team, announcement })
            }
            ind @ (SpecialVictory::Independent(_) | SpecialVictory::Hunted) => Some(ind),
        }
    }

    fn apply_forfeits(
        &self,
        holder: usize,
        ctx: &mut WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        (**self).apply_forfeits(holder, ctx, winners);
    }
}

impl dyn Role {
    pub fn id(&self) -> &'static str {
        self.info().id
    }

    pub fn name(&self) -> &'static str {
        self.info().name
    }

    pub fn game(&self) -> GameSet {
        self.info().game
    }

    pub fn team(&self) -> Team {
        self.info().team
    }

    pub fn threat_team(&self) -> Option<Team> {
        self.info().threat_team
    }

    pub fn night_faction(&self) -> Option<NightFaction> {
        self.info().night_faction
    }

    pub fn aliases(&self) -> &'static [&'static str] {
        self.info().aliases
    }

    pub fn is_minion(&self) -> bool {
        self.info().is_minion
    }

    pub fn places_marks(&self) -> bool {
        self.info().places_marks
    }

    pub fn requires_pair_in_deck(&self) -> bool {
        self.info().requires_pair_in_deck
    }

    pub fn anarchy_bundle(&self) -> Option<&'static str> {
        self.info().anarchy_bundle
    }

    pub fn extra_non_village_labels(&self) -> &'static [&'static str] {
        self.info().extra_non_village_labels
    }

    pub fn styled_name(&self) -> Text {
        self.info().styled_name()
    }

    /// True for primary Werewolf/Vampire/Alien/SuperVillain threats (including `Groob` & `Zerb`, excluding `Minion` and `Renfield`).
    pub fn is_primary_threat(&self) -> bool {
        self.threat_team().is_some()
    }

    /// True for roles that cause `Paranormal Investigator` to stop & transform and `Revealer`
    /// to flip a card back face-down (monster threats and `Independent` roles, excluding minions).
    pub fn triggers_pi_and_revealer(&self) -> bool {
        self.team() != Team::Village && !self.is_minion()
    }

    pub fn has_step(&self, step: &WakeStep) -> bool {
        let target_id = step.id();
        self.steps().iter().any(|st| st.id() == target_id)
    }

    pub fn is_doppelganger(&self) -> bool {
        self.steps().iter().any(|st| st.is_doppelganger)
    }

    /// Returns the step executed in `DoppelgangerImmediate` if `Doppelgänger` acts immediately on copying `self`.
    pub fn immediate_copy_step(&self) -> Option<&'static WakeStep> {
        self.steps()
            .iter()
            .find(|st| !st.is_doppelganger && st.copy_timing == Some(CopyTiming::Immediate))
    }

    pub fn doppelganger_acts_immediately(&self) -> bool {
        self.immediate_copy_step().is_some()
    }

    /// True if `Doppelgänger` copying this role will wake later during Dusk or Night on schedule.
    pub fn doppelganger_wakes_on_schedule(&self) -> bool {
        if self.doppelganger_acts_immediately() {
            return false;
        }
        self.steps()
            .iter()
            .any(|st| !st.is_doppelganger && st.order > 10)
    }
}

impl fmt::Debug for dyn Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Role({})", self.name())
    }
}

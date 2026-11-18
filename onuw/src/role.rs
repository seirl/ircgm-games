//! [`Role`] trait, [`RoleInfo`], [`NightFaction`], [`WakeStep`] schedule metadata,
//! [`EndgameModifier`] pipeline, and access-checked Dusk/Night hook contexts.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

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
    /// *Team Tanner* (`Tanner`, `Apprentice Tanner`, `Artifact::CudgelOfTheTanner`).
    Tanner,
    /// *Team Mad* (`Mad Scientist`, `Intern`).
    Mad,
    /// Solo / unaffiliated custom win condition (`Blob`, `Mortician`, `Assassin`, `Synthetic Alien`, `Family Man`, rival `Groob`/`Zerb`).
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

    pub const fn is_monster(self) -> bool {
        let mut i = 0;
        while i < Self::MONSTER_TEAMS.len() {
            if self as u8 == Self::MONSTER_TEAMS[i] as u8 {
                return true;
            }
            i += 1;
        }
        false
    }

    pub const fn is_collective(self) -> bool {
        matches!(self, Team::Village) || self.is_monster()
    }

    pub fn label(self) -> &'static str {
        match self {
            Team::Village => "Village",
            Team::Werewolf => "Werewolf",
            Team::Vampire => "Vampire",
            Team::Alien => "Alien",
            Team::SuperVillain => "Super Villain",
            Team::Tanner => "Tanner",
            Team::Mad => "Team Mad",
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
            Team::Tanner => "Tanner Team",
            Team::Mad => "Mad Team",
            Team::Independent => "Independent",
        }
    }

    pub fn style_text(self, s: &'static str) -> Text {
        match self {
            Team::Village => s.bold().cyan().into(),
            Team::Werewolf | Team::SuperVillain => s.bold().red().into(),
            Team::Vampire => s.bold().magenta().into(),
            Team::Alien => s.bold().green().into(),
            Team::Tanner | Team::Mad | Team::Independent => s.bold().orange().into(),
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
        let threat_team = if team.is_monster() { Some(team) } else { None };
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
    /// Copied action runs silently in the shared `DoppelgangerImmediate` window right after `Doppelgänger` (`#1` / `#-7`).
    Immediate,
    /// Copied action runs in a dedicated `<Copier>-<Role>` tier right after `Doppelgänger` (`Doppelgänger-Oracle`).
    AfterPlayerCopier,
    /// Copied action runs in the role's scheduled Dusk/Night tier as a separate `#X-A` sub-wake after initial holders.
    OnRoleTurn,
    /// Copied actor wakes simultaneously inside the shared open-eyed group (`Mason`, `Groob & Zerb`, `Intern`, `Apprentice Assassin`).
    WithGroup,
}

/// Optional once-per-role setup and epilogue hooks for [`WakeStep::solo`] and [`WakeStep::passive`]
/// roles whose Mother Ship / announcer directive rolls once and announces publicly even when unheld
/// (`Body Snatcher`, `Psychic`, `Rascal`, `Exposer`, `Family Man`, `Blob`, `Mortician`).
#[derive(Clone, Copy)]
pub struct TierHooks {
    pub before_first_slot: fn(&mut BoardMut<'_>, &Roster),
    pub after_last_slot: fn(&mut BoardMut<'_>, &mut ResolveCtx<'_>),
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

/// Whether a [`WakeStep`] copies another player's card (`Doppelgänger`) or a center card (`Copycat`, `Mirror Man`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopierKind {
    Player,
    Center,
}

/// Declarative Dusk/Night schedule entry contributed by [`Role::steps`].
///
/// Defaults across all constructors:
/// - `eyes_open = true` (override with `.eyes_closed()` for eyes-closed roles like `Cow` and `Evilometer`).
/// - `prompt_reads = VISIBLE_TABLE` for interactive steps (extend with `.reads(...)` when reading other zones mid-prompt).
/// - `resolve_writes = NONE` (set with `.writes(...)` when mutating board state at resolution time).
/// - `copy_timing = Immediate` for [`WakeStep::solo`] (override with `.copy_on_turn()` when `Doppelgänger` waits for the role's scheduled turn), `OnRoleTurn` for `group`/`passive`, `None` for `pack`/`player_copier`.
#[derive(Clone, Copy)]
pub struct WakeStep {
    target: StepTarget,
    pub phase: Phase,
    pub eyes_open: bool,
    pub short_budget: bool,
    pub copier: Option<CopierKind>,
    pub run_when_unheld: bool,
    pub copy_timing: Option<CopyTiming>,
    pub prompt_reads: BoardAccess,
    pub resolve_writes: BoardAccess,
    pub(crate) tier_hooks: Option<TierHooks>,
    pub(crate) kind: StepKind,
}

impl fmt::Debug for WakeStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WakeStep({}, {:?})", self.callout_name(), self.phase)
    }
}

impl WakeStep {
    /// Interactive per-seat DM prompt (`eyes_open = true`, `copy_timing = Immediate` by default).
    pub const fn solo(owner: &'static dyn Role, f: SoloPromptFn) -> Self {
        Self {
            target: StepTarget::Role(owner),
            phase: Phase::Night,
            eyes_open: true,
            short_budget: false,
            copier: None,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::Immediate),
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            tier_hooks: None,
            kind: StepKind::Solo(f),
        }
    }

    /// Player-card copier (`Doppelgänger`): wakes at `#-7` when Dusk roles are in play,
    /// or `#1` (after `#0 Sentinel`) otherwise.
    pub const fn player_copier(owner: &'static dyn Role, f: SoloPromptFn) -> Self {
        let mut s = Self::solo(owner, f)
            .short_budget()
            .writes(BoardAccess::PLAYER_BECAME | BoardAccess::WAKING_ROLES);
        s.copier = Some(CopierKind::Player);
        s.copy_timing = None;
        s
    }

    /// Center-card copier (`Copycat`, `Mirror Man`): views 1 center card on a short pick budget
    /// and adopts that role for the rest of the game.
    pub const fn center_copier(owner: &'static dyn Role, f: SoloPromptFn) -> Self {
        let mut s = Self::solo(owner, f)
            .short_budget()
            .copy_on_turn()
            .writes(BoardAccess::PLAYER_BECAME | BoardAccess::WAKING_ROLES);
        s.copier = Some(CopierKind::Center);
        s
    }

    /// Role-owned group wake (`Mason`, `Intern`, `Apprentice Assassin`, `Oracle`, `Nostradamus`, `Empath`).
    ///
    /// Defaults to `CopyTiming::WithGroup` (override with `.copy_on_turn()` or `.copy_after_player_copier()`
    /// when `Doppelgänger` wakes separately from the base role).
    pub const fn group(owner: &'static dyn Role, f: GroupPromptFn) -> Self {
        Self {
            target: StepTarget::Role(owner),
            phase: Phase::Night,
            eyes_open: true,
            short_budget: false,
            copier: None,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::WithGroup),
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            tier_hooks: None,
            kind: StepKind::Group(f),
        }
    }

    /// Shared monster-pack wake (`Werewolves`, `Vampires`, `Aliens`, `Super Villains`).
    pub const fn pack(faction: NightFaction, f: GroupPromptFn) -> Self {
        Self {
            target: StepTarget::Pack(faction),
            phase: Phase::Night,
            eyes_open: true,
            short_budget: false,
            copier: None,
            run_when_unheld: false,
            copy_timing: None,
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            tier_hooks: None,
            kind: StepKind::Group(f),
        }
    }

    /// Shared pair/rival group wake (`Groob & Zerb` `#1D`).
    pub const fn pair(
        id: &'static str,
        callout: &'static str,
        team: Team,
        faction: Option<NightFaction>,
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
            eyes_open: true,
            short_budget: false,
            copier: None,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::WithGroup),
            prompt_reads: BoardAccess(BoardAccess::VISIBLE_TABLE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            tier_hooks: None,
            kind: StepKind::Group(f),
        }
    }

    /// Passive resolution-time wake (`eyes_open = true` by default; chain `.eyes_closed()` for `Cow`/`Evilometer`).
    pub const fn passive(owner: &'static dyn Role, f: PassiveWakeFn) -> Self {
        Self {
            target: StepTarget::Role(owner),
            phase: Phase::Night,
            eyes_open: true,
            short_budget: false,
            copier: None,
            run_when_unheld: false,
            copy_timing: Some(CopyTiming::OnRoleTurn),
            prompt_reads: BoardAccess(BoardAccess::NONE),
            resolve_writes: BoardAccess(BoardAccess::NONE),
            tier_hooks: None,
            kind: StepKind::Passive(f),
        }
    }

    pub const fn run_when_unheld(mut self) -> Self {
        assert!(
            matches!(self.kind, StepKind::Group(_)),
            "only Group steps support .run_when_unheld() (use .with_tier_hooks(...) on Solo/Passive steps)"
        );
        self.run_when_unheld = true;
        self
    }

    pub const fn with_tier_hooks(
        mut self,
        before_first_slot: fn(&mut BoardMut<'_>, &Roster),
        after_last_slot: fn(&mut BoardMut<'_>, &mut ResolveCtx<'_>),
    ) -> Self {
        assert!(
            matches!(self.kind, StepKind::Solo(_) | StepKind::Passive(_)),
            "only Solo and Passive steps support .with_tier_hooks(...)"
        );
        self.run_when_unheld = true;
        self.tier_hooks = Some(TierHooks {
            before_first_slot,
            after_last_slot,
        });
        self
    }

    pub const fn copy_after_player_copier(mut self) -> Self {
        assert!(
            matches!(self.kind, StepKind::Group(_)),
            "only Group steps support .copy_after_player_copier()"
        );
        self.copy_timing = Some(CopyTiming::AfterPlayerCopier);
        self
    }

    pub const fn copy_immediately(mut self) -> Self {
        assert!(
            matches!(self.kind, StepKind::Solo(_) | StepKind::Passive(_)),
            "only Solo and Passive steps support .copy_immediately()"
        );
        self.copy_timing = Some(CopyTiming::Immediate);
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
            matches!(self.kind, StepKind::Solo(_) | StepKind::Group(_)),
            "only Solo and Group steps support .copy_on_turn() (Passive steps already default to OnRoleTurn)"
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

    /// True if this step runs code at tier start (an interactive DM prompt or `TierHooks::before_first_slot`).
    pub const fn has_tier_start(&self) -> bool {
        !matches!(self.kind, StepKind::Passive(_)) || self.tier_hooks.is_some()
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
    /// every step with a tier-start phase implicitly depends on `WAKING_ROLES | PUBLIC_ANNOUNCE`
    /// (even with eyes closed), plus `VISIBLE_TABLE` when `eyes_open`.
    pub fn effective_prompt_reads(&self) -> u16 {
        if !self.has_tier_start() {
            return BoardAccess::NONE;
        }
        let vis = if self.eyes_open {
            BoardAccess::VISIBLE_TABLE
        } else {
            BoardAccess::NONE
        };
        self.prompt_reads.0 | BoardAccess::WAKING_ROLES | BoardAccess::PUBLIC_ANNOUNCE | vis
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

    pub fn player_copied_seats(&self) -> SeatSet {
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
        if !copied.is_player_copier() && !copied.is_center_copier() {
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
    is_immediate_copy: bool,
    dm: Option<ExclusiveDm>,
    pub roster: &'a Roster,
    pub ask_cfg: &'a AskConfig,
    pub deadline: Instant,
    wakers: &'a NightWakers,
    board: PromptBoard<'a>,
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

    pub fn is_center_face_up(self, slot: usize) -> bool {
        self.check_read(BoardAccess::FACE_UP, "FACE_UP");
        self.board.raw().is_center_face_up(slot)
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
        if eyes_open {
            if let Some(dm_ref) = &dm {
                if let Some(summary) = board.raw().visible_table_summary(roster) {
                    let _ = dm_ref.say(summary);
                }
            }
        }
        Self {
            step_name,
            reads,
            seat,
            is_immediate_copy: false,
            dm,
            roster,
            ask_cfg,
            deadline,
            wakers,
            board,
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
        match &mut self.dm {
            Some(dm) => dm.ask(prompt, matcher).config(self.ask_cfg).await,
            None => Ok(Answer::Left {
                nick: self.nick(self.seat),
                reason: None,
            }),
        }
    }

    pub fn say(&self, text: impl Into<Text>) {
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

    pub fn is_center_face_up(&self, slot: usize) -> bool {
        self.check_read(BoardAccess::FACE_UP, "FACE_UP");
        self.board.raw().is_center_face_up(slot)
    }

    /// Reads `players[seat]` mid-prompt (`Paranormal Investigator`).
    pub fn peek_player_card(&self, seat: usize) -> Card {
        self.check_read(
            BoardAccess::PLAYER_FACES | BoardAccess::SHIELDS,
            "PLAYER_FACES | SHIELDS",
        );
        self.board.raw().player_card(seat)
    }

    /// Snapshot of board cards for roles that already declare `PLAYER_FACES` (`Nostradamus`).
    pub(crate) fn board_for_peek(&self) -> &BoardCards {
        self.check_read(BoardAccess::PLAYER_FACES, "PLAYER_FACES");
        self.board.raw()
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

    /// All seats holding `role` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
    pub fn seats_with_role(&self, role: &'static dyn Role) -> SeatSet {
        self.wakers.seats_with_role(role)
    }

    /// Waking seats holding `faction` (excluding seats suppressed by `Mark::Fear`).
    pub fn waking_seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.waking_seats_with_faction(faction)
    }

    /// Packmates known to `self.seat` at the time of this prompt:
    /// - After the pack wake (`OnRoleTurn`), all waking members of `faction`.
    /// - Before the pack wake (`DoppelgangerImmediate` at `#1`), only `self.seat` and the player
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
    is_player_copy_turn: bool,
    pub roster: &'a Roster,
    pub ask_cfg: &'a AskConfig,
    pub deadline: Instant,
    pub waking_roles: &'a [&'static dyn Role],
    wakers: &'a NightWakers,
    board: PromptBoard<'a>,
}

impl<'a> GroupPromptCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        step_name: &'static str,
        reads: u16,
        eyes_open: bool,
        active_seats: SeatSet,
        roster: &'a Roster,
        ask_cfg: &'a AskConfig,
        deadline: Instant,
        wakers: &'a NightWakers,
        board: PromptBoard<'a>,
    ) -> Self {
        if eyes_open {
            if let Some(summary) = board.raw().visible_table_summary(roster) {
                for seat in active_seats {
                    let _ = roster.seats[seat].user.say(summary.clone());
                }
            }
        }
        Self {
            step_name,
            reads,
            is_player_copy_turn: false,
            roster,
            ask_cfg,
            deadline,
            waking_roles: wakers.roles(),
            wakers,
            board,
        }
    }

    pub(crate) fn as_player_copy_turn(mut self) -> Self {
        self.is_player_copy_turn = true;
        self
    }

    /// True when invoked in the dedicated `CopyTiming::AfterPlayerCopier` tier (`Doppelgänger-Oracle`).
    pub fn is_player_copy_turn(&self) -> bool {
        self.is_player_copy_turn
    }

    #[inline]
    fn check_read(&self, mask: u16, zone: &str) {
        assert!(
            (self.reads & mask) == mask,
            "{}: undeclared group prompt read of {zone} — add .reads(BoardAccess::{zone}) to WakeStep",
            self.step_name
        );
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
        self.roster.seats[seat]
            .user
            .ask(prompt, matcher)
            .config(self.ask_cfg)
            .await
    }

    pub fn dm(&self, seat: usize, text: impl Into<Text>) {
        let _ = self.roster.seats[seat].user.say(text);
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

    /// All seats holding `faction` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
    pub fn seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.seats_with_faction(faction)
    }

    /// All seats holding `role` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
    pub fn seats_with_role(&self, role: &'static dyn Role) -> SeatSet {
        self.wakers.seats_with_role(role)
    }

    /// Waking seats holding `faction` (excluding seats suppressed by `Mark::Fear`).
    pub fn waking_seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.waking_seats_with_faction(faction)
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

    /// Acquires a per-seat [`PromptCtx`] inside a group step (`Nostradamus`).
    pub async fn for_seat(&self, seat: usize) -> Result<PromptCtx<'a>> {
        let dm = self.roster.seats[seat].user.exclusive_available().await?;
        Ok(PromptCtx {
            step_name: self.step_name,
            reads: self.reads,
            seat,
            is_immediate_copy: false,
            dm,
            roster: self.roster,
            ask_cfg: self.ask_cfg,
            deadline: self.deadline,
            wakers: self.wakers,
            board: self.board,
        })
    }
}

pub struct ResolveCtx<'a> {
    pub roster: &'a Roster,
    pub chronicle: &'a mut Vec<Text>,
    pub(crate) room: &'a Room,
    pub(crate) wakers: &'a mut NightWakers,
    pub(crate) step_name: &'static str,
    pub(crate) writes: u16,
}

impl<'a> ResolveCtx<'a> {
    pub fn record(&mut self, entry: impl Into<Text>) {
        self.chronicle.push(entry.into());
    }

    pub fn announce(&self, text: impl Into<Text>) {
        assert!(
            (self.writes & BoardAccess::PUBLIC_ANNOUNCE) == BoardAccess::PUBLIC_ANNOUNCE,
            "{}: undeclared write to PUBLIC_ANNOUNCE — add .writes(BoardAccess::PUBLIC_ANNOUNCE) to WakeStep",
            self.step_name
        );
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

    /// All seats holding `faction` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
    pub fn seats_with_faction(&self, faction: NightFaction) -> SeatSet {
        self.wakers.seats_with_faction(faction)
    }

    /// All seats holding `role` (initial role or copied by `Copycat` / `Doppelgänger`), even if feared.
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

    pub fn grant_wake_log(&mut self, seat: usize) {
        self.wakers.wake_log_recipients.insert(seat);
    }
}

pub struct DeathCtx<'a> {
    pub roster: &'a Roster,
    pub board: &'a BoardCards,
    pub votes: &'a [usize],
    pub tally: &'a [usize],
    pub max_votes: usize,
}

pub enum VoteDefense {
    /// Votes against `seat` cannot execute them (`Prince`, `The Sponge`, `Bodyguard`, `Defender-er`, `The Master`);
    /// the runner-up with `>= 2` votes dies instead.
    Immune {
        seat: usize,
        /// True if this defense also blocks `Hunter`-style death shots (`Bodyguard`, `Defender-er`, `The Master`),
        /// false for vote-only immunity (`Prince`, `The Sponge`).
        blocks_hunter_shot: bool,
        announcement: Text,
    },
    /// Votes against `from` still count for top-vote comparison, and if `from` would be executed by the vote,
    /// execution bounces onto `to` instead (`Ricochet Rhino`).
    Ricochet {
        from: usize,
        to: usize,
        announcement: Text,
    },
}

impl VoteDefense {
    pub fn immune(seat: usize, blocks_hunter_shot: bool, announcement: impl Into<Text>) -> Self {
        Self::Immune {
            seat,
            blocks_hunter_shot,
            announcement: announcement.into(),
        }
    }

    pub fn ricochet(from: usize, to: usize, announcement: impl Into<Text>) -> Self {
        Self::Ricochet {
            from,
            to,
            announcement: announcement.into(),
        }
    }
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
    pub votes: &'a [usize],
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
            Self::MonsterTeams => team.is_monster(),
            Self::AllTeams => team.is_collective(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SpecialVictory {
    /// Personal victory for an `Independent` role or artifact holder, optionally blocking teams from winning.
    Independent(TeamBlock),
    /// Teamless role (`Innocent Bystander`) that is not on `team` (e.g. for `Traitor` / `Flipper` / `Nostradamus`),
    /// yet wins whenever `team` wins.
    WithTeam(Team),
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

    pub const fn with_team(team: Team) -> Self {
        Self::WithTeam(team)
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
    pub votes: &'a [usize],
    pub dead_seats: SeatSet,
    pub team_block: TeamBlock,
    pub winning_teams: &'a std::collections::HashSet<Team>,
    pub base_winners: &'a WinnerMap,
    pub hunted_active: bool,
    pub hunted_seats: SeatSet,
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

    fn vote_defense(&self, _seat: usize, _ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        None
    }

    fn on_death(&self, _dead_seat: usize, _ctx: &DeathCtx<'_>) -> Vec<DeathEffect> {
        Vec::new()
    }

    fn victory_claims(&self, _seat: usize, _ctx: &VictoryCtx<'_>) -> Vec<SpecialVictory> {
        Vec::new()
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

/// Object-safe role trait implemented by all 80 cards across Base + 5 Expansions.
pub trait Role: Send + Sync + 'static {
    fn info(&self) -> RoleInfo;

    fn summary_for_deck(&self, _roster: &Roster) -> String {
        self.info().summary.to_owned()
    }

    fn steps(&self) -> &'static [WakeStep] {
        &[]
    }

    /// Optional Voting-time conversion triggered by who voted for `seat` (`Cursed`, `Windy Wendy`).
    fn vote_conversion(
        &self,
        _seat: usize,
        _roster: &Roster,
        _board: &BoardCards,
        _votes: &[usize],
    ) -> Option<(&'static dyn Role, Text)> {
        None
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        _board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur
    }

    fn vote_defense(&self, _seat: usize, _ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        None
    }

    fn on_death(&self, _dead_seat: usize, _ctx: &DeathCtx<'_>) -> Option<DeathEffect> {
        None
    }

    fn special_victory(&self, _seat: usize, _ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        None
    }

    fn special_victories(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Vec<SpecialVictory> {
        self.special_victory(seat, ctx).into_iter().collect()
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

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        (**self).vote_defense(seat, ctx)
    }

    fn on_death(&self, dead_seat: usize, ctx: &DeathCtx<'_>) -> Vec<DeathEffect> {
        (**self).on_death(dead_seat, ctx).into_iter().collect()
    }

    fn victory_claims(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Vec<SpecialVictory> {
        // Respect `Role < Mark < Artifact` precedence: if a higher-priority modifier changed this
        // seat's effective team away from the role's own team, the role's victory claims do not apply.
        let role_team = (**self)
            .adjust_alignment(
                seat,
                ctx.board,
                EffectiveAlignment {
                    team: self.team(),
                    threat_team: self.threat_team(),
                },
            )
            .team;
        if ctx.board.effective_team(seat) != role_team {
            return Vec::new();
        }
        (**self).special_victories(seat, ctx)
    }

    fn override_win(
        &self,
        holder: usize,
        ctx: &WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        (**self).override_win(holder, ctx, winners);
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

    pub fn has_step(&self, step: &WakeStep) -> bool {
        let target_id = step.id();
        self.steps().iter().any(|st| st.id() == target_id)
    }

    pub fn is_player_copier(&self) -> bool {
        self.steps()
            .iter()
            .any(|st| st.copier == Some(CopierKind::Player))
    }

    pub fn is_center_copier(&self) -> bool {
        self.steps()
            .iter()
            .any(|st| st.copier == Some(CopierKind::Center))
    }

    /// Returns the step executed in `DoppelgangerImmediate` if a player-copier acts immediately on copying `self`.
    pub fn immediate_copy_step(&self) -> Option<&'static WakeStep> {
        self.steps().iter().find(|st| {
            st.copier != Some(CopierKind::Player) && st.copy_timing == Some(CopyTiming::Immediate)
        })
    }

    pub fn has_after_player_copier_step(&self) -> bool {
        self.steps()
            .iter()
            .any(|st| st.copy_timing == Some(CopyTiming::AfterPlayerCopier))
    }
}

impl fmt::Debug for dyn Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Role({})", self.name())
    }
}

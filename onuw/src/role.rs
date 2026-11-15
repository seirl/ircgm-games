//! Clean 7-method [`Role`] trait, [`RoleInfo`], [`Wake`] schedule metadata, and hook contexts.

use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use ircgm::prelude::*;
use ircgm::IntoAskMatch;
use tokio::time::Instant;

use crate::board::{BoardCards, Roster};

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
    /// Werewolves, Aliens, Super Villains, and Minion.
    Werewolf,
    /// Vampires (`Vampire`, `The Master`, `Count`), Renfield, and `Mark::Vampire` thralls.
    Vampire,
    /// Solo / custom win condition (`Tanner`, `Assassin`, `Groob`, `Zerb`, `Blob`, `Mortician`, `Mad Scientist`, `Intern`).
    Independent,
}

impl Team {
    pub fn label(self) -> &'static str {
        match self {
            Team::Village => "Village",
            Team::Werewolf => "Werewolf",
            Team::Vampire => "Vampire",
            Team::Independent => "Independent",
        }
    }

    pub fn styled_label(self) -> Text {
        match self {
            Team::Village => "Village".bold().cyan(),
            Team::Werewolf => "Werewolf".bold().red(),
            Team::Vampire => "Vampire".bold().magenta(),
            Team::Independent => "Independent".bold().orange(),
        }
    }
}

/// Cross-role pack memberships and multi-role capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoleTag {
    /// Waking Werewolves (`Werewolf`, `Alpha Wolf`, `Mystic Wolf`) — wake together & seen by `Minion`.
    WolfPack,
    /// Sleeping Werewolf (`Dream Wolf`) — seen by `Minion`, does not wake with `WolfPack`.
    DreamWolfThumb,
    /// Waking Vampires (`Vampire`, `The Master`, `Count`) — wake together in Dusk & seen by `Renfield`.
    VampirePack,
    /// Aliens (`Alien`, `Groob`, `Zerb`, `Synthetic Alien`) — seen by `Aliens`, `Leader`, and `Cow`.
    AlienPack,
    /// Super Villains (`Henchman`, `Temptress`, `Dr. Peeker`, `Rapscallion`) — wake together & trigger `Evilometer`.
    VillainPack,
    /// Role bestows non-Clarity Vampire `Mark`s.
    PlacesMarks,
}

/// Static card identity, team alignment, and deck metadata returned by [`Role::info`].
#[derive(Clone, Copy, Debug)]
pub struct RoleInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub game: GameSet,
    pub team: Team,
    pub summary: &'static str,
    pub aliases: &'static [&'static str],
    pub tags: &'static [RoleTag],
    pub is_minion: bool,
    pub requires_pair_in_deck: bool,
}

impl RoleInfo {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        game: GameSet,
        team: Team,
        summary: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            game,
            team,
            summary,
            aliases: &[],
            tags: &[],
            is_minion: false,
            requires_pair_in_deck: false,
        }
    }

    pub const fn aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }

    pub const fn tags(mut self, tags: &'static [RoleTag]) -> Self {
        self.tags = tags;
        self
    }

    pub const fn minion(mut self) -> Self {
        self.is_minion = true;
        self
    }

    pub const fn paired(mut self) -> Self {
        self.requires_pair_in_deck = true;
        self
    }

    pub fn styled_name(self) -> Text {
        match self.team {
            Team::Werewolf => self.name.bold().red().into(),
            Team::Vampire => self.name.bold().magenta().into(),
            Team::Independent => self.name.bold().orange().into(),
            Team::Village => self.name.bold().cyan().into(),
        }
    }
}

// ============================================================================
// Dusk / Night Schedule & Typed Resolution Closures
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyBehavior {
    Sleep,
    ActImmediately,
    JoinGroupWake,
    WakeAfterAllMutations,
}

/// Explicit phase in the Dusk/Night schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WakePhase {
    /// Runs before Copycats (`Sentinel`).
    PreCopycatDusk,
    /// Copycat / Doppelgänger phase (`Copycat` then `Doppelgänger`).
    Copycat,
    /// Vampire-expansion Dusk phase.
    Dusk,
    /// Standard Night phase.
    Night,
}

/// Deferred resolution closure returned by a solo role's DM prompt (`WakeKind::Solo`),
/// invoked at the fixed-clock tier deadline with `(actor_label, &mut BoardCards, &mut ResolveCtx)`.
pub struct SeatEffect(Box<dyn FnOnce(&str, &mut BoardCards, &mut ResolveCtx<'_>) + Send>);

impl SeatEffect {
    pub fn new(
        f: impl FnOnce(&str, &mut BoardCards, &mut ResolveCtx<'_>) + Send + 'static,
    ) -> Self {
        Self(Box::new(f))
    }

    pub fn noop() -> Self {
        Self::new(|_, _, _| {})
    }

    pub fn apply(self, actor_label: &str, board: &mut BoardCards, ctx: &mut ResolveCtx<'_>) {
        (self.0)(actor_label, board, ctx);
    }
}

/// Deferred resolution closure returned by a group/pack role's prompt (`WakeKind::Group`),
/// invoked at the fixed-clock tier deadline with `(&mut BoardCards, &mut ResolveCtx)`.
pub struct GroupEffect(Box<dyn FnOnce(&mut BoardCards, &mut ResolveCtx<'_>) + Send>);

impl GroupEffect {
    pub fn new(f: impl FnOnce(&mut BoardCards, &mut ResolveCtx<'_>) + Send + 'static) -> Self {
        Self(Box::new(f))
    }

    pub fn noop() -> Self {
        Self::new(|_, _| {})
    }

    pub fn apply(self, board: &mut BoardCards, ctx: &mut ResolveCtx<'_>) {
        (self.0)(board, ctx);
    }
}

/// Deferred resolution closure returned by `Copycat` / `Doppelgänger` (`WakeKind::Copycat`),
/// invoked at the CopycatPick tier deadline.
pub struct CopycatEffect(
    Box<dyn FnOnce(&mut BoardCards, &mut ResolveCtx<'_>) -> Option<CopiedRecord> + Send>,
);

impl CopycatEffect {
    pub fn new(
        f: impl FnOnce(&mut BoardCards, &mut ResolveCtx<'_>) -> Option<CopiedRecord>
            + Send
            + 'static,
    ) -> Self {
        Self(Box::new(f))
    }

    pub fn apply(self, board: &mut BoardCards, ctx: &mut ResolveCtx<'_>) -> Option<CopiedRecord> {
        (self.0)(board, ctx)
    }
}

pub type SoloPromptFn = for<'a> fn(usize, PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>>;
pub type PassiveWakeFn = fn(usize, &str, &mut BoardCards, &mut ResolveCtx<'_>);
pub type GroupPromptFn =
    for<'a> fn(Vec<usize>, GroupPromptCtx<'a>) -> BoxFuture<'a, Result<GroupEffect>>;
pub type CopycatPromptFn =
    for<'a> fn(usize, PromptCtx<'a>) -> BoxFuture<'a, Result<CopycatEffect>>;

/// How the scheduler dispatches a waking role.
#[derive(Clone, Copy)]
pub enum WakeKind {
    /// Interactive per-seat DM prompt returning a [`SeatEffect`].
    Solo(SoloPromptFn),
    /// Passive per-seat wake at resolution time with no DM input (`Insomniac`, `Cow`, `Evilometer`).
    Passive(PassiveWakeFn),
    /// Shared group wake returning a [`GroupEffect`], optionally collecting all seats with `pack`.
    Group {
        pack: Option<RoleTag>,
        prompt: GroupPromptFn,
    },
    /// Copycat / Doppelgänger card-selection wake, falling back to `fallback` if unawakened.
    Copycat {
        fallback: &'static dyn Role,
        pick: CopycatPromptFn,
    },
}

impl fmt::Debug for WakeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solo(_) => write!(f, "Solo"),
            Self::Passive(_) => write!(f, "Passive"),
            Self::Group { pack, .. } => write!(f, "Group({pack:?})"),
            Self::Copycat { fallback, .. } => write!(f, "Copycat({})", fallback.name()),
        }
    }
}

/// Complete Dusk/Night schedule entry for a role (`None` on [`Role::wake`] means the role sleeps).
#[derive(Clone, Copy, Debug)]
pub struct Wake {
    pub phase: WakePhase,
    pub order: u16,
    pub kind: WakeKind,
    pub mutates_board: bool,
    /// True for interactive roles (`Count`, `Paranormal Investigator`, `Witch`, `Rogue`) whose
    /// prompt inspects `ctx.board()` mid-prompt and must start a new tier after any earlier mutator.
    pub reads_live_board: bool,
    pub copy_behavior: CopyBehavior,
}

impl Wake {
    pub const fn solo(order: u16, prompt: SoloPromptFn) -> Self {
        Self {
            phase: WakePhase::Night,
            order,
            kind: WakeKind::Solo(prompt),
            mutates_board: false,
            reads_live_board: false,
            copy_behavior: CopyBehavior::ActImmediately,
        }
    }

    pub const fn passive(order: u16, wake_fn: PassiveWakeFn) -> Self {
        Self {
            phase: WakePhase::Night,
            order,
            kind: WakeKind::Passive(wake_fn),
            mutates_board: false,
            reads_live_board: false,
            copy_behavior: CopyBehavior::WakeAfterAllMutations,
        }
    }

    pub const fn group(order: u16, prompt: GroupPromptFn) -> Self {
        Self {
            phase: WakePhase::Night,
            order,
            kind: WakeKind::Group { pack: None, prompt },
            mutates_board: false,
            reads_live_board: false,
            copy_behavior: CopyBehavior::JoinGroupWake,
        }
    }

    pub const fn pack(order: u16, tag: RoleTag, prompt: GroupPromptFn) -> Self {
        Self {
            phase: WakePhase::Night,
            order,
            kind: WakeKind::Group {
                pack: Some(tag),
                prompt,
            },
            mutates_board: false,
            reads_live_board: false,
            copy_behavior: CopyBehavior::JoinGroupWake,
        }
    }

    pub const fn copycat(
        order: u16,
        fallback: &'static dyn Role,
        pick: CopycatPromptFn,
    ) -> Self {
        Self {
            phase: WakePhase::Copycat,
            order,
            kind: WakeKind::Copycat { fallback, pick },
            mutates_board: true,
            reads_live_board: true,
            copy_behavior: CopyBehavior::Sleep,
        }
    }

    pub const fn dusk(mut self) -> Self {
        self.phase = WakePhase::Dusk;
        self
    }

    pub const fn before_copycat(mut self) -> Self {
        self.phase = WakePhase::PreCopycatDusk;
        self
    }

    /// Indicates that this role mutates cards, marks, shields, artifacts, or state on `BoardCards`.
    pub const fn mutates(mut self) -> Self {
        self.mutates_board = true;
        self
    }

    /// Enables `ctx.board()` during the prompt and forces a tier split after any earlier mutator.
    pub const fn reads_board(mut self) -> Self {
        self.reads_live_board = true;
        self
    }

    /// Marks a passive role as acting immediately when copied (`Psychic`).
    pub const fn immediate_on_copy(mut self) -> Self {
        self.copy_behavior = CopyBehavior::ActImmediately;
        self
    }

    pub const fn needs_dm_prompt(self) -> bool {
        !matches!(self.kind, WakeKind::Passive(_))
    }
}

// ============================================================================
// Night & Dawn Hook Contexts
// ============================================================================

#[derive(Clone, Copy)]
pub struct CopiedRecord {
    pub copycat_seat: usize,
    pub copied_role: &'static dyn Role,
}

fn seats_with_tag_in(roster: &Roster, copied_records: &[CopiedRecord], tag: RoleTag) -> Vec<usize> {
    let mut out: Vec<usize> = roster
        .seats
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.initial_role.tags().contains(&tag).then_some(i))
        .collect();
    for rec in copied_records {
        if rec.copied_role.tags().contains(&tag) && !out.contains(&rec.copycat_seat) {
            out.push(rec.copycat_seat);
        }
    }
    out.sort_unstable();
    out
}

pub struct PromptCtx<'a> {
    pub(crate) seat: usize,
    pub(crate) dm: Option<ExclusiveDm>,
    pub roster: &'a Roster,
    pub ask_cfg: &'a AskConfig,
    pub deadline: Instant,
    pub shielded: &'a HashSet<usize>,
    pub copied_records: &'a [CopiedRecord],
    pub(crate) live_board: Option<&'a BoardCards>,
}

impl<'a> PromptCtx<'a> {
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

    /// Live pre-tier `BoardCards` (available only when `.reads_board()` was declared on `Wake`).
    pub fn board(&self) -> &'a BoardCards {
        self.live_board.expect(
            "role called `ctx.board()` during prompt without `.reads_board()` on `Wake`",
        )
    }

    /// All seats holding `tag` (initial role or copied by a Copycat/Doppelgänger).
    pub fn seats_with_tag(&self, tag: RoleTag) -> Vec<usize> {
        seats_with_tag_in(self.roster, self.copied_records, tag)
    }
}

pub struct GroupPromptCtx<'a> {
    pub bot: &'a Bot,
    pub roster: &'a Roster,
    pub ask_cfg: &'a AskConfig,
    pub deadline: Instant,
    pub copied_records: &'a [CopiedRecord],
    pub shielded: &'a HashSet<usize>,
    pub feared_seats: &'a HashSet<usize>,
    pub(crate) live_board: Option<&'a BoardCards>,
}

impl<'a> GroupPromptCtx<'a> {
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

    /// Opens a drop-out-tolerant DM [`Huddle`] across `seats` using [`Bot::huddle_available`].
    pub async fn huddle(&self, name: impl Into<Text>, seats: &[usize]) -> Result<Huddle> {
        let users: Vec<User> = seats
            .iter()
            .map(|&s| self.roster.seats[s].user.clone())
            .collect();
        self.bot.huddle_available(name, &users).await
    }

    /// Opens a drop-out-tolerant DM [`Huddle`] across `seats` and keeps its pump-dispatched
    /// relay active until the tier deadline.
    pub async fn relay_huddle(&self, name: impl Into<Text>, seats: &[usize]) -> Result<()> {
        let _huddle = self.huddle(name, seats).await?;
        tokio::time::sleep_until(self.deadline).await;
        Ok(())
    }

    pub fn nick(&self, seat: usize) -> String {
        self.roster.seats[seat].user.nick()
    }

    /// Live pre-tier `BoardCards` (available only when `.reads_board()` was declared on `Wake`).
    pub fn board(&self) -> &'a BoardCards {
        self.live_board.expect(
            "group role called `ctx.board()` during prompt without `.reads_board()` on `Wake`",
        )
    }

    /// All seats holding `tag` (initial role or copied by a Copycat/Doppelgänger), even if feared.
    pub fn seats_with_tag(&self, tag: RoleTag) -> Vec<usize> {
        seats_with_tag_in(self.roster, self.copied_records, tag)
    }

    /// Waking seats holding `tag` (excluding seats suppressed by `Mark::Fear`).
    pub fn waking_seats_with_tag(&self, tag: RoleTag) -> Vec<usize> {
        self.seats_with_tag(tag)
            .into_iter()
            .filter(|s| !self.feared_seats.contains(s))
            .collect()
    }

    /// All seats holding `role` (initial role or copied by a Copycat/Doppelgänger).
    pub fn seats_with_role(&self, role: &'static dyn Role) -> Vec<usize> {
        let target_id = role.id();
        let mut out: Vec<usize> = self
            .roster
            .seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (s.initial_role.id() == target_id).then_some(i))
            .collect();
        for rec in self.copied_records {
            if rec.copied_role.id() == target_id && !out.contains(&rec.copycat_seat) {
                out.push(rec.copycat_seat);
            }
        }
        out.sort_unstable();
        out
    }
}

pub struct ResolveCtx<'a> {
    pub roster: &'a Roster,
    pub chronicle: &'a mut Vec<Text>,
}

impl<'a> ResolveCtx<'a> {
    pub fn record(&mut self, entry: impl Into<Text>) {
        self.chronicle.push(entry.into());
    }

    pub fn nick(&self, seat: usize) -> String {
        self.roster.seats[seat].user.nick()
    }

    pub fn dm(&self, seat: usize, text: impl Into<Text>) {
        let _ = self.roster.seats[seat].user.say(text);
    }
}

pub struct DeathCtx<'a> {
    pub roster: &'a Roster,
    pub board: &'a BoardCards,
    pub votes: &'a [Option<usize>],
}

pub struct VoteProtection {
    pub protected_seat: usize,
    /// True if this protection also blocks `Hunter`-style death shots (`Bodyguard`), false for vote-only immunity (`Prince`, `The Master`).
    pub blocks_hunter_shot: bool,
    pub announcement: Text,
}

pub enum DeathEffect {
    /// Immediately kills another seat (`Hunter`), cascading in the Dawn fixpoint loop.
    Kill { seat: usize, announcement: Text },
    /// Prevents the listed seats from winning (`Diseased` neighbors).
    ForfeitWins(Vec<usize>),
}

pub struct VictoryCtx<'a> {
    pub roster: &'a Roster,
    pub board: &'a BoardCards,
    pub votes: &'a [Option<usize>],
    pub dead_seats: &'a HashSet<usize>,
}

impl<'a> VictoryCtx<'a> {
    pub fn is_dead(&self, seat: usize) -> bool {
        self.dead_seats.contains(&seat)
    }

    pub fn find_seats(&self, role: &'static dyn Role) -> Vec<usize> {
        let target_id = role.id();
        (0..self.roster.len())
            .filter(|&i| self.board.players[i].effective_role().id() == target_id)
            .collect()
    }

    pub fn find_seats_with_tag(&self, tag: RoleTag) -> Vec<usize> {
        (0..self.roster.len())
            .filter(|&i| self.board.players[i].effective_role().tags().contains(&tag))
            .collect()
    }
}

#[derive(Clone, Debug)]
pub enum SpecialVictory {
    /// Personal victory for an `Independent` role holder, optionally blocking teams from winning.
    Independent { blocks: &'static [Team] },
    /// Overrides normal team victory rules so `team` wins (`Leader` unanimous Alien point).
    ForceTeam { team: Team, announcement: Text },
}

impl SpecialVictory {
    pub const fn independent() -> Self {
        Self::Independent { blocks: &[] }
    }

    pub const fn independent_blocking(blocks: &'static [Team]) -> Self {
        Self::Independent { blocks }
    }

    pub fn force_team(team: Team, announcement: impl Into<Text>) -> Self {
        Self::ForceTeam {
            team,
            announcement: announcement.into(),
        }
    }
}

// ============================================================================
// Object-Safe 7-Method `Role` Trait
// ============================================================================

/// Object-safe role trait implemented by all 60 cards across Base + 4 Expansions.
pub trait Role: Send + Sync + 'static {
    fn info(&self) -> RoleInfo;

    fn summary_for_deck(&self, _roster: &Roster) -> String {
        self.info().summary.to_owned()
    }

    fn on_deal(&self, _seat: usize, _board: &mut BoardCards, _ctx: &mut ResolveCtx<'_>) {}

    fn wake(&self) -> Option<Wake> {
        None
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

    pub fn night_summary(&self) -> &'static str {
        self.info().summary
    }

    pub fn aliases(&self) -> &'static [&'static str] {
        self.info().aliases
    }

    pub fn tags(&self) -> &'static [RoleTag] {
        self.info().tags
    }

    pub fn is_minion(&self) -> bool {
        self.info().is_minion
    }

    pub fn requires_pair_in_deck(&self) -> bool {
        self.info().requires_pair_in_deck
    }

    pub fn styled_name(&self) -> Text {
        self.info().styled_name()
    }

    /// True for primary Werewolf/Alien/Villain/Vampire threats (excluding `Minion` and `Renfield`).
    pub fn is_primary_threat(&self) -> bool {
        matches!(self.team(), Team::Werewolf | Team::Vampire) && !self.is_minion()
    }

    pub fn copy_behavior(&self) -> CopyBehavior {
        if let Some(w) = self.wake() {
            w.copy_behavior
        } else if self.tags().iter().any(|t| {
            matches!(
                t,
                RoleTag::WolfPack | RoleTag::VampirePack | RoleTag::VillainPack
            )
        }) {
            CopyBehavior::JoinGroupWake
        } else {
            CopyBehavior::Sleep
        }
    }

    pub fn copycat_fallback(&self) -> Option<&'static dyn Role> {
        match self.wake()?.kind {
            WakeKind::Copycat { fallback, .. } => Some(fallback),
            _ => None,
        }
    }

    pub fn is_copycat(&self) -> bool {
        self.copycat_fallback().is_some()
    }
}

impl fmt::Debug for dyn Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Role({})", self.name())
    }
}

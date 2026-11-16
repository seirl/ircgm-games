# ONUW Role, Board & Scheduler Refactoring Plan

## Why We Are Refactoring

The original `Role` trait (`onuw/src/role.rs`) modeled Base ONUW ("1 card per seat, at most 1 Night wake, card-only Voting hooks"), which leaked across 4 expansions in five ways:

1. **`RoleTag` conflated three orthogonal jobs:** pack-wake scheduling (`WolfPack` pulling `WEREWOLF` into the schedule via an `ALL_ROLES` scan), Night visibility (`DreamWolfThumb` bolted on because `Dream Wolf` sleeps while still appearing as a Wolf), and deck capability (`PlacesMarks`). Meanwhile `Role::wake() -> Option<Wake>` allowed at most one wake per card, forcing two-step specialists (`Alpha Wolf`, `Mystic Wolf`, `Count`, `Temptress`, `Dr. Peeker`, `Rapscallion`, `Groob`, `Zerb`) to smuggle their pack wake via `RoleTag`.
2. **`Role` was extensible, while `Mark` and `Artifact` were hardcoded switches:** `board.rs` and `voting.rs` accumulated bespoke branches for `Mark::Vampire`, `Mark::Love`, `Mark::Traitor`, `Mark::Disease`, `"diseased"`, `"renfield"`, `"groob"`, `"zerb"`, `"leader"`, `CudgelOfTheTanner`, `Mask of Muting`, and `Shroud of Shame`. `VictoryCtx::find_seats` also missed `Artifact`/`Mark::Vampire` overrides, causing `Apprentice Assassin` to diverge from `Groob`/`Zerb`/`Intern`.
3. **`BoardCards` exposed raw `pub` fields and expansion-specific globals:** `oracle_omen`, `apprentice_assassin_marked`, `extra_evil_card`, `extra_villain_card`, and `viewed_or_moved_actors` lived directly on `BoardCards`, and every role had to manually check `shielded` twice and remember `record_card_interaction` for `Aura Seer`.
4. **Coarse `.mutates()` / `.reads_board()` flags caused unnecessary 25s tier splits:** Because the entire `BoardCards` was treated as a single dependency bit, roles touching disjoint tabletop zones were serialized (e.g. in 4p *Artifact Chaos*, `Paranormal Investigator` only writes `card.became` while `Witch`'s prompt only reads `center[0..3]`, yet they ran in separate 25s tiers; `Psychic` reading `oracle_omen` split after `Alien`/`Alpha Wolf`; `Curator` split into a solo 25s tier at the end of almost every game).
5. **Unused vs. missing hooks:** `Role::on_deal` had zero implementations across all 59 roles, while dynamic alignment, post-vote win forfeits, and deck-hazard metadata had no trait representation (forcing hardcoded ID lists in `board.rs` and `scenarios.rs`).

---

## Agreed Design Decisions

- **Minimal role-body churn now:** Keep existing `ctx.ask(...)` prompt closures intact (see `future_prompt_combinators.md` for the deferred follow-up); only migrate raw `board.*` reads/writes to access-checked `PromptCtx` and `BoardMut` methods.
- **Core vocabulary:**
  - `EndgameModifier`: unified post-vote alignment, immunity, death-chain, victory, win-override, and win-forfeit pipeline implemented across `Role` (via `impl EndgameModifier for &'static dyn Role` so role files keep a single `impl Role` block), `Mark`, and `Artifact`.
  - `NightFaction`: `Option<NightFaction>` (`Werewolf`, `Vampire`, `Alien`, `SuperVillain`) on `RoleInfo`, replacing `RoleTag` and `DreamWolfThumb` (`Dream Wolf` simply has `night_faction: Some(NightFaction::Werewolf)` and `&[]` `WakeStep`s).
  - `WakeStep`: roles return `&'static [WakeStep]` (`0`, `1`, or `2` steps, deduplicated by step ID), with shared pack steps (`WOLF_PACK_STEP`, `VAMPIRE_PACK_STEP`, `ALIEN_PACK_STEP`, `VILLAIN_PACK_STEP`) and ergonomic defaults (`eyes_open = true`, `reads = VISIBLE_TABLE`, `writes = NONE`, `copy_timing = OnRoleTurn`, overridden only when needed via `.eyes_closed()`, `.reads(...)`, `.writes(...)`, `.immediate_copy()`, `.dusk()`).
  - `BoardAccess` + `BoardMut<'a>`: zone-level bitmask checked on access inside `PromptCtx`, `GroupPromptCtx`, and `BoardMut` over module-private `BoardCards` fields (`#![forbid(unsafe_code)]`), keeping role files free of const-generic/trait ceremony while catching any undeclared read/write immediately in tests.
  - `BoardKey` (`TypeId` blackboard on `BoardCards`): moves `OracleOmen`, `ApprenticeSoloMark`, `CenterWolfCard`, and `ExtraVillainCard` into their own role modules so `board.rs` has zero expansion-specific fields or role imports.
- **Gameplay & visibility semantics:**
  - **Pure pack huddles:** Huddles always relay everything as pure chat; pack specialists (`Alpha Wolf`, `Mystic Wolf`, `Temptress`, `Dr. Peeker`, `Rapscallion`, `Count`) run in the tier after their pack huddle so hidden actions never leak to packmates.
  - **Tabletop visibility (`🛡`, `🔦 Role`, `🏺`):** Every `eyes_open: true` step automatically includes `VISIBLE_TABLE` (`SHIELDS | FACE_UP | ARTIFACTS`) in its effective prompt read mask—creating natural sequence points only after `Sentinel` (`100`), `Revealer` (`192`), and `Curator` (`194`) when present—and renders visible markers **only when at least one is actually on the table** (no empty-state DMs). Sleeping roles (`Dream Wolf`, `Villager`, etc.) and `eyes_open: false` roles (`Cow`, `Evilometer`) never see visible markers or cause visibility splits.
  - **Rule cleanups:**
    1. `Curator` may place Artifacts on cards flipped face-up by `Revealer` (official Daybreak rule; fixes the Night DM info-leak and merges `Curator` into the main tier in non-`Revealer` games while showing `🔦 Role` to `Curator` when `Revealer` is in play).
    2. `Doppelgänger -> Mystic Wolf` wakes on Mystic Wolf's post-pack turn (`CopyTiming::OnRoleTurn`, `#2C`).
    3. `Thing` and `Annoying Lad` both use `CopyTiming::Immediate`.
    4. `Apprentice Assassin` uses the unified override-aware `board.active_holders(&ASSASSIN)` query.
  - **Intra-expansion coupling:** Coupled sibling roles inside the same `src/roles/<expansion>/` directory may reference each other directly (`Alien` spotting `Cow`, `Villains` spotting `Evilometer`, `Leader` checking `Groob`/`Zerb`), while the engine (`board.rs`, `night.rs`, `voting.rs`, `deck.rs`, `scenarios.rs`) becomes 100% role-agnostic.

---

## Core Type Signatures

### 1. `BoardAccess`, `PromptCtx`, `BoardMut`, and `BoardKey` (`src/board.rs`, `src/role.rs`)

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoardAccess(pub u16);

impl BoardAccess {
    pub const NONE: u16          = 0;
    pub const PLAYER_FACES: u16  = 1 << 0; // card identity sitting in players[seat]
    pub const PLAYER_BECAME: u16 = 1 << 1; // in-place transform (PI, Copycat, Doppelgänger)
    pub const CENTER_FACES: u16  = 1 << 2; // center[0..3]
    pub const FACE_UP: u16       = 1 << 3; // card.face_up (Revealer)
    pub const SHIELDS: u16       = 1 << 4; // Sentinel shield tokens
    pub const MARKS: u16         = 1 << 5; // Vampire Dusk marks
    pub const ARTIFACTS: u16     = 1 << 6; // Curator artifact tokens
    pub const WAKING_ROLES: u16  = 1 << 7; // Copycat / Doppelgänger waking identity
    pub const CUSTOM_0: u16      = 1 << 8; // Blackboard slot: OracleOmen
    pub const CUSTOM_1: u16      = 1 << 9; // Blackboard slot: CenterWolfCard / ExtraVillainCard / ApprenticeSoloMark

    pub const VISIBLE_TABLE: u16 = Self::SHIELDS | Self::FACE_UP | Self::ARTIFACTS;
}

pub trait BoardKey: Clone + std::fmt::Debug + Send + Sync + 'static {
    const ACCESS: u16;
    fn init() -> Self;
}
```

- All fields on `BoardCards` are private to `board.rs` (with minimal `#[cfg(test)] pub(crate)` inspection helpers for `testing.rs`).
- Every accessor on `PromptCtx<'a>`, `PromptView<'a>`, and `GroupPromptCtx<'a>` asserts `(self.reads & mask) == mask` with the role's name in the error message.
- Every mutator on `BoardMut<'a>` asserts `(self.writes & mask) == mask` (plus `SHIELDS` read access on interactive player-card operations), automatically checking `shielded` and recording `viewed_or_moved_actors` for `Aura Seer`.

### 2. `NightFaction` & `WakeStep` (`src/role.rs`, `src/night.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NightFaction {
    Werewolf,
    Vampire,
    Alien,
    SuperVillain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyTiming {
    /// Copied action fires in the DoppelgangerImmediate window right after Doppelgänger (#1 / #-7).
    Immediate,
    /// Copied action fires on the copied role's turn right after initial holders (X-A).
    OnRoleTurn,
}

pub struct WakeStep {
    pub id: &'static str, // deduplicates shared pack steps (e.g. "pack:werewolf")
    pub callout_name: &'static str,
    pub styled_callout: fn() -> Text,
    pub phase: Phase,
    pub order: u16,
    pub eyes_open: bool,
    pub short_budget: bool,
    pub is_doppelganger: bool,
    pub copy_timing: Option<CopyTiming>, // None on shared pack steps (joined via NightFaction)
    pub prompt_reads: BoardAccess,
    pub resolve_writes: BoardAccess,
    pub kind: StepKind,
}
```

**Scheduler Tier-Splitting Rule (`build_deck_schedule`):**
A `WakeStep` starts a new `ScheduledTier` iff:
1. `phase` changes (`Dusk` -> `Night`),
2. `short_budget` is true (`Copycat`, `Doppelgänger`), or `count_in_deck > 1` with `(effective_prompt_reads & resolve_writes) != 0` (`RoleSlot` for `2x Witch`, `2x Curator`, etc.),
3. The current tier contains a pack step whose `NightFaction` matches this role's `night_faction` (keeps huddles pure and specialists post-huddle), or
4. `step.needs_dm_prompt() && (cur_tier_writes & effective_prompt_reads) != 0`, where `effective_prompt_reads` automatically includes `VISIBLE_TABLE` whenever `step.eyes_open` is true. (Passive `eyes_open` wakes like `Insomniac`, `Aura Seer`, and `Blob` have no prompt phase—they execute synchronously at resolution time in canonical `order`, reading live `BoardCards` state directly.)

For `TierKind::DoppelgangerImmediate`, `resolve_writes` is the union of `resolve_writes` across all `CopyTiming::Immediate` steps present in `deck`.

### 3. `EndgameModifier` & Self-Describing `RoleInfo` (`src/role.rs`, `src/marks.rs`, `src/artifacts.rs`, `src/voting.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectiveAlignment {
    pub team: Team,
    pub threat_team: Option<Team>,
}

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
    fn override_win(&self, _holder: usize, _ctx: &WinnerFilterCtx<'_>, _winners: &mut WinnerMap) {}
    fn apply_forfeits(
        &self,
        _holder: usize,
        _ctx: &mut WinnerFilterCtx<'_>,
        _winners: &mut WinnerMap,
    ) {
    }
}
```

- `Role` removes the unused `on_deal` method, replaces `wake() -> Option<Wake>` with `steps(&self) -> &'static [WakeStep]`, and adds `adjust_alignment` + `apply_forfeits` (bridged to `EndgameModifier` via `impl EndgameModifier for &'static dyn Role` so role files still write a single `impl Role`).
- `RoleInfo` replaces `tags: &'static [RoleTag]` with:
  - `night_faction: Option<NightFaction>`
  - `places_marks: bool`
  - `requires_pair_in_deck: bool` (`Mason`)
  - `anarchy_bundle: Option<&'static str>` (`Some("groob_zerb")` on `Groob` & `Zerb`)
  - `extra_non_village_labels: &'static [&'static str]` (`Curator` -> `&["Werewolf", "Tanner"]`, `Groob`/`Zerb` -> `&["Alien"]`)

---

## Phased Implementation Slices

Each slice leaves the entire workspace compiling and `cargo test` green before moving to the next:

1. **Slice 1 — `EndgameModifier` Pipeline + Self-Describing `RoleInfo`:**
   - Unify `Role`, `Mark`, and `Artifact` endgame hooks (`adjust_alignment`, `vote_protection`, `on_death`, `victory_claim`, `modify_winners`).
   - Move `Renfield`, `Groob`, `Zerb`, `Leader`, and `Diseased` special cases out of `board.rs` / `voting.rs` into their own role files, and `Love` / `Traitor` / `Disease` / `Cudgel` / `Mute` / `Shame` into `marks.rs` and `artifacts.rs`.
   - Add `board.active_holders(role)` and `RoleInfo` deck metadata; delete all hardcoded role-ID checks in `board.rs`, `voting.rs`, and `scenarios.rs`; delete unused `Role::on_deal`.
2. **Slice 2 — `NightFaction` + Multi-Step `WakeStep` + Dusk `Mark` Hooks:**
   - Replace `RoleTag` (`DreamWolfThumb`, `PlacesMarks`, `*Pack`) with `NightFaction` + `places_marks: bool`.
   - Replace `Role::wake() -> Option<Wake>` with `Role::steps() -> &'static [WakeStep]` and shared pack steps (`WOLF_PACK_STEP`, `VAMPIRE_PACK_STEP`, `ALIEN_PACK_STEP`, `VILLAIN_PACK_STEP`).
   - Make `CopyTiming` explicit (`Mystic Wolf` -> `OnRoleTurn`; `Thing` & `Annoying Lad` -> `Immediate`) and move Dusk-end `Fear`/`Love` notifications onto `Mark`.
3. **Slice 3 — Private `BoardCards`, `BoardKey` Blackboard, Compile-Time `BoardAccess` & Smarter Tier Packing:**
   - Add `TypeId` blackboard (`BoardKey`) to `BoardCards` and move `OracleOmen`, `ApprenticeSoloMark`, `CenterWolfCard`, and `ExtraVillainCard` into their respective role modules.
   - Make `BoardCards` fields private to `board.rs`; introduce `PromptCtx<'a, READS>`, `GroupPromptCtx<'a, READS>`, and `BoardMut<'a, READS, WRITES>` with `const { assert!(...) }` checks and automatic shield + `Aura Seer` tracking.
   - Remove `Curator`'s `is_player_face_up` restriction; render non-empty visible tabletop markers (`🛡`, `🔦 Role`, `🏺`) to `eyes_open: true` wakes; switch `build_deck_schedule` to `BoardAccess` conflict detection.
4. **Slice 4 — Schedule & Regression Verification:**
   - Add tests covering *Artifact Chaos* tier merging (`PI + Witch + Village Idiot + Aura Seer + Revealer` in one tier, `Curator` seeing `🔦` when `Revealer` is in play and merging when `Revealer` is absent), `Curator` placing an Artifact onto a face-up card, eyes-closed roles (`Dream Wolf`, `Cow`, `Evilometer`) never seeing tabletop markers, empty tabletop markers producing zero extra DMs, and `Doppelgänger -> Mystic Wolf` waking at `#2C`.

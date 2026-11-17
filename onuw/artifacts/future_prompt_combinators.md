# Future Idea: Composable Prompt-Picker Combinators & Counterpart Helpers

*(Deferred follow-up to `role_abstraction_plan.md`, updated after implementing all 80 roles across Base + 4 Expansions & Bonus Packs 1–4.)*

## Problem Statement

Even after `BoardCards` mutations, `BoardAccess` checks, and `pointing.rs` (`silent_point` / `SeatPair`) are unified, ~55 of the 80 role files under `onuw/src/roles/` repeat the same 40–100 lines of IRC DM input parsing inside `ctx.ask(...)` (and *Super Villains* Heroes + *Alien*'s `Rascal` duplicate full action flows of *Base* / *Daybreak* counterparts):

- Trimming input and matching `"pass"` / `"skip"` / `"none"` (`Roster::is_pass`)
- Resolving player nicknames via `roster.resolve_nick(...)`
- Rejecting self-targeting (`"Pick ANOTHER player, not yourself!"`)
- Rejecting shielded cards (`"🛡️ That player's card is shielded!"`)
- Rejecting already face-up cards (`Revealer`, `Flipper`) or already-artifacted seats (`Curator`)
- Rejecting already-inspected seats in multi-peek loops (`Paranormal Investigator`, `Nostradamus`)
- Restricting to a candidate `SeatSet` (`Psychic` parity seats, `Count` non-Vampire/non-thrall targets, `Body Snatcher` / `Alpha Wolf` / `Temptress` non-packmates, `Squire` / `Beholder` thumb-up targets)
- Parsing 1 center slot (`1`, `#1`, `c1`, `center 1`) or `k` distinct center slots (`Seer`, `Detector`, `Exposer`)
- Parsing `left` / `l` / `right` / `r` / `<neighbor_nick>` (`Thing`, `Annoying Lad`, `Diseased`, `Psychic`, `Mortician`) or rotation direction (`Village Idiot`, `Rascal::Idiot`)
- Parsing two distinct nicknames `<nick1> <nick2>` via `SeatPair` (`Troublemaker`, `Switcheroo`, `Rascal::Troublemaker`, `Cupid`)
- Falling back to a random valid target (or self / right neighbor) on timeout for mandatory roles (`Doppelgänger`, `Copycat`, `Mirror Man`, `Drunk`, `Rascal::Drunk`, `Body Snatcher`, `Witch` / `Voodoo Lou` / `Rascal::Witch` step 2, `Alpha Wolf`, `Temptress`, `Count`, `Diseased`, `Assassin`)

## Design Principle: Combinators Over Rigid Archetype DSLs

A rigid declarative DSL (e.g. `WakeStep::peek_one_player(PeekFlavor { ... })`) gets awkward whenever a role needs slightly different phrasing, conditional sub-prompts (`Marksman`, `Gremlin`, `Psychic`, `Rascal`, `Nostradamus`), or custom chronicle formatting.

Instead, layer **two thin, opt-in abstractions**:
1. **Composable picker methods on `PromptCtx<'a>`** (handling validation, shield/self/faction/subset rejection, `"pass"` parsing, and timeout fallbacks while keeping custom `Text` prompts/acks in each role). Because these pickers live on `PromptCtx<'a>`, they automatically enforce runtime `BoardAccess` checks (`self.check_read(...)`) on the underlying `WakeStep`!
2. **Small parameterized flow helpers** (e.g. in `src/role_flows.rs`, alongside `src/pointing.rs`) ONLY for the 1-to-1 counterpart pairs/trios whose rules and resolution steps are genuinely identical.

---

## Recurring Picker Shapes Across All 80 Roles

| Combinator | Roles Using This Input Shape |
| :--- | :--- |
| **1 player (`pick_player`)** | `Mystic Wolf`, `Dr. Peeker`, `Robber`, `Rascal` (`Robber` & `Witch` step 2), `Role Retriever`, `Revealer`, `Flipper`, `Curator`, `Sentinel`, `Doppelgänger`, `Alpha Wolf`, `Temptress`, `Body Snatcher` (`StealPlayer`), `Count`, `Witch` (step 2), `Voodoo Lou` (step 2), `Paranormal Investigator` (loop 1..=2), `Nostradamus` (loop 1..=3), `Pickpocket`, `Priest`, `Instigator`, `Assassin` |
| **2 distinct players (`pick_pair`)** | `Troublemaker`, `Switcheroo`, `Rascal` (`Troublemaker`), `Cupid` (+ sub-branches of `Gremlin`, `Alien`) |
| **1 center slot (`pick_center`)** | `Copycat`, `Mirror Man`, `Apprentice Seer`, `Rapscallion`, `Drunk`, `Rascal` (`Drunk` & `Witch` step 1), `Body Snatcher` (`StealCenter`), `Werewolf` (Lone Wolf), `Witch` (step 1), `Voodoo Lou` (step 1) |
| **`k` center slots (`pick_centers`)** | `Exposer` (`k = 1..=3` face-down, all-or-nothing + `"all"` shorthand), center branch of `Seer` & `Detector` (`k = 2`) |
| **Player OR 2 centers (`pick_player_or_two_centers`)** | `Seer`, `Detector` |
| **Subset / thumb inspect (`pick_subset_or_all`)** | `Squire` (thumb-up `Werewolves`: `all` / `yes` / `<nick>...` / `pass`), `Beholder` (thumb-up `Seer` & `Apprentice Seer`) |
| **Neighbor (`pick_neighbor`)** | `Thing`, `Annoying Lad`, `Diseased`, `Psychic` (`ChooseNeighbor`), `Mortician` (`ChooseNeighbor`) |
| **Table rotation (`pick_rotation`)** | `Village Idiot`, `Rascal` (`Idiot` mode: `left`/`l`/`-1`, `right`/`r`/`+1`, or `pass`) |

*(Bespoke multi-grammar roles like `Marksman`, `Gremlin`, and `Oracle` continue calling raw `ctx.ask(...)` directly or composing smaller token parsers.)*

---

## Updated `PromptCtx` Picker API Sketch

```rust
#[derive(Clone, Copy)]
pub struct TargetFilter {
    pub allow_self: bool,
    pub respect_shields: bool,
    pub exclude_face_up: bool,
    pub exclude_existing_artifact: bool,
    pub exclude_faction: Option<NightFaction>,
    pub exclude_seats: SeatSet,
    pub allowed_seats: Option<SeatSet>,
}

impl TargetFilter {
    /// Another unshielded player (`Robber`, `Mystic Wolf`, `Dr. Peeker`, `Doppelgänger`, `Role Retriever`).
    pub const fn other_unshielded() -> Self {
        Self {
            allow_self: false,
            respect_shields: true,
            exclude_face_up: false,
            exclude_existing_artifact: false,
            exclude_faction: None,
            exclude_seats: SeatSet::EMPTY,
            allowed_seats: None,
        }
    }

    /// Any unshielded player including self (`Witch` / `Voodoo Lou` / `Rascal::Witch` step 2, `Curator`).
    pub const fn any_unshielded() -> Self {
        Self {
            allow_self: true,
            ..Self::other_unshielded()
        }
    }

    /// Any player ignoring shields, e.g. Dusk Mark placement (`Instigator`, `Assassin`) or `Pickpocket`.
    pub const fn any_player_for_mark(allow_self: bool) -> Self {
        Self {
            allow_self,
            respect_shields: false,
            ..Self::other_unshielded()
        }
    }

    /// For `Revealer` and `Flipper`.
    pub const fn excluding_face_up(mut self) -> Self {
        self.exclude_face_up = true;
        self
    }

    /// For `Curator`.
    pub const fn excluding_artifacted(mut self) -> Self {
        self.exclude_existing_artifact = true;
        self
    }

    /// Excludes known waking packmates (`Alpha Wolf`, `Temptress`, `Body Snatcher`).
    pub const fn excluding_faction(mut self, faction: NightFaction) -> Self {
        self.exclude_faction = Some(faction);
        self
    }

    /// Excludes previously inspected seats in multi-peek loops (`PI`, `Nostradamus`).
    pub const fn excluding_seats(mut self, seats: SeatSet) -> Self {
        self.exclude_seats = seats;
        self
    }

    /// Restricts valid choices to a precomputed whitelist (`Count`, `Psychic` parity, `Squire`, `Beholder`).
    pub const fn only_in(mut self, seats: SeatSet) -> Self {
        self.allowed_seats = Some(seats);
        self
    }
}

pub enum Fallback {
    /// Optional action: accepts `pass` / `skip`, and defaults to `None` on timeout.
    Optional { pass_ack: Text },
    /// Mandatory action: rejects `pass`, and picks a random valid candidate on timeout.
    RandomValid,
    /// Mandatory action: rejects `pass`, and prefers `actor` on timeout if valid, else random valid (`Witch`, `Voodoo Lou`).
    SelfOrRandomValid,
    /// Mandatory action: rejects `pass`, and picks right neighbor on timeout (`Assassin`).
    RightNeighbor,
}

pub struct PickOutcome<T> {
    pub chosen: Option<T>,
    pub timed_out_random: bool,
}
```

### Example: `Mystic Wolf` With `pick_player`

Note that `PromptCtx::new` already emits `👀 On the table: ...` automatically when `eyes_open == true`, so prompts only format plain `ctx.roster.format_players()`:

```rust
async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
    let picked = ctx
        .pick_player(
            TargetFilter::other_unshielded(),
            irc_format!(
                "🐺🔮 Mystic Wolf: Reply {} to inspect another player's card, or {}. Players: {}",
                "<nick>".bold(),
                "pass".bold(),
                ctx.roster.format_players(),
            ),
            |target_nick| {
                irc_format!(
                    "🐺🔮 Locked in inspecting {} — card reveals when the phase ends!",
                    target_nick.bold(),
                )
            },
            Fallback::Optional {
                pass_ack: "🐺🔮 Passed — not inspecting anyone.".into(),
            },
        )
        .await?;

    Ok(SeatEffect::new(move |label, board, rctx| {
        if let Some(target) = picked.chosen {
            if let Some(card) = board.peek_player(seat, target) {
                let target_nick = rctx.nick(target);
                rctx.dm(
                    seat,
                    irc_format!("🐺🔮 {}'s card is: {}", target_nick.bold(), card.display_for_peek()),
                );
                rctx.record(irc_format!(
                    "🐺🔮 {label} ({}) viewed {}'s card ({}).",
                    rctx.nick(seat).bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
    }))
}
```

### Access-Check Safety Tie-In

- Calling `ctx.pick_player(filter, ...)` runs `self.check_read(BoardAccess::SHIELDS, "SHIELDS")` when `filter.respect_shields`, `BoardAccess::FACE_UP` when `filter.exclude_face_up`, and `BoardAccess::ARTIFACTS` when `filter.exclude_existing_artifact`.
- Calling `ctx.pick_centers(k, /*only_face_down=*/ true, ...)` runs `self.check_read(BoardAccess::FACE_UP, "FACE_UP")`.
- If a mandatory role (`Fallback::RandomValid` / `SelfOrRandomValid`) has zero valid targets (e.g. every non-Wolf or non-Alien was shielded), `pick_player` returns `PickOutcome { chosen: None, timed_out_random: false }` cleanly without hanging or panicking on an empty set.

---

## Shared Flow Helpers for Exact Counterpart Roles

With all 80 roles implemented, several pairs and trios share 100% identical state transitions, differing only in emoji/role label:

1. **`copy_center_flow(seat, ctx, self_role, icon)`** — shared by `Copycat` (`#8`) and `Mirror Man` (`#9`).
2. **`seer_scan_flow(seat, ctx, icon, verb)`** — shared by `Seer` (`#5`) and `Detector` (`#5` Hero).
3. **`witch_brew_flow(seat, ctx, icon, mode_banner)`** — shared by `Witch`, `Voodoo Lou`, and `Rascal` (`RascalMode::Witch`).
4. **`blind_swap_two_flow(seat, ctx, icon, mode_banner)`** — shared by `Troublemaker`, `Switcheroo`, and `Rascal` (`RascalMode::Troublemaker`).
5. **`reveal_if_village_flow(seat, ctx, icon)`** — shared by `Revealer` and `Flipper` (both delegate flip-back rules to `BoardMut::reveal_player_if_village` / `Role::flips_back_on_reveal`).
6. **`self_card_check(seat, label, board, rctx, icon)`** — shared by `Insomniac` and `Self-Awareness Girl`.
7. **`thumb_group_card_check(seat, ctx, thumbs, group_label, icon)`** — shared by `Squire` (`Werewolves`) and `Beholder` (`Seer` / `Apprentice Seer`).
8. **`sense_neighbor_faction(seat, label, board, rctx, faction, hit_dm, miss_dm)`** — shared by `Cow` (`NightFaction::Alien`) and `Evilometer` (`NightFaction::SuperVillain`).
9. **`spot_mentor_group_wake(...)`** — shared recognition structure across `Apprentice Tanner` (`Tanner`), `Intern` (`Mad Scientist`), and `Apprentice Assassin` (`Assassin`).
10. **`announce_neighbor_cluster(...)`** — shared eyes-closed end-of-night cluster announcement structure between `Blob` and `Family Man`.

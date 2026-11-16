# Future Idea: Composable Prompt-Picker Combinators

*(Deferred follow-up to `role_abstraction_plan.md` — deliberately shelved until after the core `EndgameModifier` / `WakeStep` / `BoardAccess` / `BoardMut` refactoring lands.)*

## Problem Statement

Even after `BoardCards` mutations and `BoardAccess` checks are unified, ~40 of the 59 role files under `onuw/src/roles/` repeat the same 40–80 lines of IRC DM input parsing inside `ctx.ask(...)`:

- Trimming input and matching `"pass"` / `"skip"` / `"none"` (`Roster::is_pass`)
- Resolving player nicknames via `roster.resolve_nick(...)`
- Rejecting self-targeting (`"Pick ANOTHER player, not yourself!"`)
- Rejecting shielded cards (`"🛡️ That player's card is shielded!"`)
- Rejecting packmates (`Alpha Wolf`, `Temptress`, `Alien`, `Count`, `Vampire`)
- Parsing center slots (`1`, `#1`, `c1`, `center 1` via `Roster::parse_center_slot`)
- Parsing `left` / `l` / `right` / `r` / `<neighbor_nick>` (`Thing`, `Annoying Lad`, `Diseased`, `Psychic`, `Mortician`)
- Parsing two distinct nicknames `<nick1> <nick2>` (`Troublemaker`, `Cupid`, `Gremlin`, `Alien`)
- Falling back to a random valid target (or neighbor) on timeout for mandatory roles (`Doppelgänger`, `Copycat`, `Drunk`, `Alpha Wolf`, `Temptress`, `Count`, `Diseased`, `Assassin`)

## Design Principle: Combinators Over Role-Archetype DSLs

A rigid declarative DSL (e.g. `WakeStep::peek_one_player(PeekFlavor { ... })`) gets awkward fast whenever one role wants slightly different phrasing, conditional sub-prompts (`Marksman`, `Gremlin`, `Psychic`), or custom chronicle formatting.

Instead, keep normal `prompt` and `resolve` functions in each role file, and add **composable picker methods on `PromptCtx<'a, READS>`** that handle validation, shield/self/faction rejection, `"pass"` parsing, and timeout fallbacks—while preserving 100% custom prompt and confirmation `Text` in each role.

Because these pickers live on `PromptCtx<'a, READS>`, they also **enforce `BoardAccess` at compile time** (`const { assert!(READS & BoardAccess::SHIELDS != 0); }`) automatically!

---

## Recurring Picker Shapes Across the 59 Roles

| Combinator | Roles Using This Exact Input Shape |
| :--- | :--- |
| **1 player (`PlayerPick`)** | `Mystic Wolf`, `Dr. Peeker`, `Robber`, `Role Retriever`, `Revealer`, `Curator`, `Sentinel`, `Doppelgänger`, `Alpha Wolf`, `Temptress`, `Count`, `Pickpocket`, `Priest`, `Instigator`, `Assassin` |
| **2 distinct players (`PairPick`)** | `Troublemaker`, `Cupid` (+ sub-branches of `Gremlin`, `Alien`) |
| **1 center slot (`CenterPick`)** | `Apprentice Seer`, `Rapscallion`, `Copycat`, `Drunk`, `Werewolf` (Lone Wolf), `Witch` (step 1) |
| **Neighbor (`NeighborPick`)** | `Thing`, `Annoying Lad`, `Diseased`, `Psychic`, `Mortician` (+ `allow_self`) |

*(Complex multi-grammar prompts like `Seer`, `Marksman`, `Gremlin`, `Alien`, and `Oracle` can continue calling raw `ctx.ask(...)` directly or compose smaller token parsers.)*

---

## Sketch of the `PromptCtx` Picker API

```rust
#[derive(Clone, Copy)]
pub struct TargetFilter {
    pub allow_self: bool,
    pub respect_shields: bool,
    pub exclude_faction: Option<NightFaction>,
    pub exclude_existing_artifact: bool,
}

impl TargetFilter {
    /// Another unshielded player (Robber, Mystic Wolf, Dr. Peeker, Revealer, Doppelgänger, Role Retriever).
    pub const fn other_unshielded() -> Self {
        Self {
            allow_self: false,
            respect_shields: true,
            exclude_faction: None,
            exclude_existing_artifact: false,
        }
    }

    /// Any player ignoring shields, e.g. Dusk Mark placement (Instigator, Assassin) or Pickpocket.
    pub const fn any_player_for_mark(allow_self: bool) -> Self {
        Self {
            allow_self,
            respect_shields: false,
            exclude_faction: None,
            exclude_existing_artifact: false,
        }
    }

    pub const fn excluding_faction(mut self, faction: NightFaction) -> Self {
        self.exclude_faction = Some(faction);
        self
    }
}

pub enum Fallback {
    /// Optional action: accepts `pass` / `skip`, and defaults to `None` on timeout.
    Optional { pass_ack: Text },
    /// Mandatory action: rejects `pass`, and picks a random valid candidate on timeout.
    RandomValid,
    /// Mandatory action: rejects `pass`, and picks right neighbor on timeout (Assassin).
    RightNeighbor,
}
```

### Example: `Mystic Wolf` Before vs. After

Instead of ~45 lines of `AskMatch` boilerplate in `mystic_wolf.rs`, `prompt` shrinks to:

```rust
async fn prompt(seat: usize, mut ctx: PromptCtx<'_, READS>) -> Result<SeatEffect<READS, WRITES>> {
    let picked = ctx
        .pick_player(
            TargetFilter::other_unshielded(),
            irc_format!(
                "🐺🔮 Mystic Wolf: Reply {} to inspect another player's card, or {}. Players: {}",
                "<nick>".bold(),
                "pass".bold(),
                ctx.format_visible_players(),
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

### Compile-Time Safety Tie-In

Notice how `pick_player` interacts with `PromptCtx<'a, READS>`:
- Calling `ctx.pick_player(...)` on a `PromptCtx<'a, READS>` asserts `READS & BoardAccess::VISIBLE_TABLE != 0` at compile time (and if `exclude_faction.is_some()`, also asserts `READS & BoardAccess::WAKING_ROLES != 0`, or if `exclude_existing_artifact`, `READS & BoardAccess::ARTIFACTS != 0`).
- If a mandatory role (`Fallback::RandomValid`) has zero valid targets (e.g. every non-wolf was shielded), `pick_player` cleanly returns `chosen: None` without prompting or panicking on an empty slice.

---

## Optional Bonus: Two Tiny Shared Helpers for Identical Mechanics

Beyond input pickers, two passive/group patterns have zero prompt variation and could optionally share a small helper function:
1. **`sense_neighbor_faction(seat, faction, hit_dm, miss_dm, hit_log, miss_log)`** — shared by `Cow` (`NightFaction::Alien`) and `Evilometer` (`NightFaction::SuperVillain`).
2. **`spot_mentor_or_act(...)`** — shared structure between `Intern` (`Mad Scientist`) and `Apprentice Assassin` (`Assassin`).

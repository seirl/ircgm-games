//! Official & expansion-ordered **One Night Ultimate** preset scenarios catalog.
//!
//! Each expansion's scenarios live in `src/scenarios/<pack>.rs`:
//! 1. **Base Set** (`scenarios/base.rs` — 8 scenarios)
//! 2. **Daybreak** (`scenarios/daybreak.rs` — 8 scenarios)
//! 3. **Vampire** (`scenarios/vampire.rs` — 8 scenarios)
//! 4. **Epic Battle** (`scenarios/epic.rs` — 27 scenarios)
//! 5. **Alien** (`scenarios/alien.rs` — 4 scenarios)
//! 6. **Super Villains** (`scenarios/super_villains.rs` — 4 scenarios)

pub mod alien;
pub mod base;
pub mod daybreak;
pub mod epic;
pub mod super_villains;
pub mod vampire;

use crate::board::{format_role_slice, Card};
use crate::role::Role;
use crate::roles::{
    base::{villager::VILLAGER, werewolf::WEREWOLF},
    super_villains::henchman::HENCHMAN,
};

/// Logical expansion-pack ordering for browsing and filtering preset scenarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScenarioPack {
    Base,
    Daybreak,
    Vampire,
    EpicBattle,
    Alien,
    SuperVillains,
}

impl ScenarioPack {
    pub const ALL: [ScenarioPack; 6] = [
        ScenarioPack::Base,
        ScenarioPack::Daybreak,
        ScenarioPack::Vampire,
        ScenarioPack::EpicBattle,
        ScenarioPack::Alien,
        ScenarioPack::SuperVillains,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Daybreak => "daybreak",
            Self::Vampire => "vampire",
            Self::EpicBattle => "epic_battle",
            Self::Alien => "alien",
            Self::SuperVillains => "super_villains",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Base => "Base Set",
            Self::Daybreak => "Daybreak (MorgenGrauen)",
            Self::Vampire => "Vampire (VampirDämmerung)",
            Self::EpicBattle => "Epic Battle",
            Self::Alien => "Alien",
            Self::SuperVillains => "Super Villains",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let key = normalize_scenario_key(raw);
        match key.as_str() {
            "base" | "baseset" | "classic" | "onuw" | "vollmondnacht" | "26681" => {
                Some(Self::Base)
            }
            "daybreak" | "db" | "morgengrauen" | "mg" | "26729" => Some(Self::Daybreak),
            "vampire" | "vamp" | "vampires" | "vampirdammerung" | "vd" | "26778" => {
                Some(Self::Vampire)
            }
            "epic" | "epicbattle" | "epicbattles" | "eb" | "26779" => Some(Self::EpicBattle),
            "alien" | "aliens" => Some(Self::Alien),
            "supervillains" | "villains" | "sv" => Some(Self::SuperVillains),
            _ => None,
        }
    }

    pub fn scenarios(self) -> &'static [Scenario] {
        match self {
            Self::Base => base::SCENARIOS,
            Self::Daybreak => daybreak::SCENARIOS,
            Self::Vampire => vampire::SCENARIOS,
            Self::EpicBattle => epic::SCENARIOS,
            Self::Alien => alien::SCENARIOS,
            Self::SuperVillains => super_villains::SCENARIOS,
        }
    }

    /// Selects the canonical default scenario in this pack for `player_count` (`3..=10`).
    pub fn default_scenario_for(self, player_count: usize) -> &'static Scenario {
        let clamped = player_count.clamp(3, 10);
        if self == Self::EpicBattle {
            let preferred_id = match clamped {
                4..=7 => "eb_epic_clash_4p",
                8..=10 => "eb_from_dusk_till_dawn",
                _ => "eb_anarchy",
            };
            if let Some(s) = self.scenarios().iter().find(|s| s.id == preferred_id) {
                return s;
            }
        }
        self.scenarios()
            .iter()
            .find(|s| s.supports_player_count(clamped))
            .expect("every ScenarioPack covers 3..=10 players")
    }

    pub fn default_extra_evil_card(self) -> Card {
        match self {
            Self::SuperVillains => Card::new(&HENCHMAN),
            _ => Card::new(&WEREWOLF),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Difficulty {
    Introduction,
    Easy,
    Medium,
    Difficult,
    Extreme,
}

impl Difficulty {
    pub fn label(self) -> &'static str {
        match self {
            Self::Introduction => "Intro",
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Difficult => "Difficult",
            Self::Extreme => "Extreme",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ScenarioRule {
    /// Deterministic base list at `min_players` (`min_players + 3` cards) plus cumulative deltas
    /// indexed by `player_count - min_players` (`deltas[0]` is always `&[]`).
    DeltaTable {
        base: &'static [&'static dyn Role],
        deltas: &'static [&'static [&'static dyn Role]],
    },
    /// Random `player_count + 3` deal from `pool`, enforcing pair invariants (`Mason`s, `Groob + Zerb`)
    /// and at most `max_evil_threats` primary monster threat cards.
    Anarchy {
        pool: &'static [&'static dyn Role],
        max_evil_threats: usize,
        rule_text: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    pub id: &'static str,
    pub pack: ScenarioPack,
    pub section: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub difficulty: Difficulty,
    pub player_range: (usize, usize),
    pub rule: ScenarioRule,
}

impl Scenario {
    pub fn min_players(&self) -> usize {
        self.player_range.0
    }

    pub fn max_players(&self) -> usize {
        self.player_range.1
    }

    pub fn supports_player_count(&self, n: usize) -> bool {
        (self.player_range.0..=self.player_range.1).contains(&n)
    }

    pub fn is_anarchy(&self) -> bool {
        matches!(self.rule, ScenarioRule::Anarchy { .. })
    }

    pub fn range_label(&self) -> String {
        let (min_p, max_p) = self.player_range;
        if min_p == max_p {
            format!("{min_p}p")
        } else {
            format!("{min_p}–{max_p}p")
        }
    }

    /// Builds an exact `(player_count + 3)`-card deck for `player_count`, returning `Err` if
    /// `player_count` is outside `self.player_range`.
    pub fn build_for_players(
        &self,
        player_count: usize,
    ) -> Result<Vec<&'static dyn Role>, String> {
        let (min_p, max_p) = self.player_range;
        if !(min_p..=max_p).contains(&player_count) {
            return Err(format!(
                "Scenario `{}` ({}) requires {min_p}–{max_p} players (currently {player_count}).",
                self.id, self.name
            ));
        }

        let target = player_count + 3;
        match self.rule {
            ScenarioRule::DeltaTable { base, deltas } => {
                let offset = player_count - min_p;
                let delta = deltas.get(offset).copied().unwrap_or(&[]);
                let mut deck = Vec::with_capacity(base.len() + delta.len());
                deck.extend_from_slice(base);
                deck.extend_from_slice(delta);
                if deck.len() != target {
                    return Err(format!(
                        "Scenario `{}` misconfigured for {player_count}p: expected {target} cards, got {}.",
                        self.id,
                        deck.len()
                    ));
                }
                Ok(deck)
            }
            ScenarioRule::Anarchy {
                pool,
                max_evil_threats,
                ..
            } => Ok(deal_anarchy_deck(pool, target, max_evil_threats)),
        }
    }

    /// Concise one-line preview for `player_count` (clamped into `player_range` for lobby preview).
    pub fn preview_cards(&self, player_count: usize) -> String {
        let clamped_n = player_count.clamp(self.player_range.0, self.player_range.1);
        match self.rule {
            ScenarioRule::DeltaTable { .. } => match self.build_for_players(clamped_n) {
                Ok(cards) => format!(
                    "{clamped_n}p ({} cards): {}",
                    cards.len(),
                    format_role_slice(&cards)
                ),
                Err(e) => e,
            },
            ScenarioRule::Anarchy { rule_text, .. } => format!("🎲 {rule_text}"),
        }
    }

    /// Detailed multi-step description for `!scenario <id>`.
    pub fn describe_lines(&self) -> Vec<String> {
        let header = format!(
            "📜 {} (`{}`) — [{} / {} | {} | {}]",
            self.name,
            self.id,
            self.pack.label(),
            self.section,
            self.difficulty.label(),
            self.range_label(),
        );
        let mut lines = vec![header];
        match self.rule {
            ScenarioRule::DeltaTable { base, deltas } => {
                let min_p = self.player_range.0;
                lines.push(format!(
                    "  • Base ({min_p}p, {} cards): {}",
                    base.len(),
                    format_role_slice(base)
                ));
                if deltas.len() > 1 {
                    let steps: Vec<String> = deltas
                        .iter()
                        .enumerate()
                        .skip(1)
                        .map(|(offset, d)| {
                            format!("{}p: +[{}]", min_p + offset, format_role_slice(d))
                        })
                        .collect();
                    lines.push(format!("  • Scaling: {}", steps.join(" | ")));
                }
            }
            ScenarioRule::Anarchy {
                pool, rule_text, ..
            } => {
                lines.push(format!("  • Rule: {rule_text}"));
                lines.push(format!(
                    "  • Pool ({} cards): {}",
                    pool.len(),
                    format_role_slice(pool)
                ));
            }
        }
        lines
    }
}

fn is_evil_threat_role(role: &'static dyn Role) -> bool {
    role.is_primary_threat()
}

fn deal_anarchy_deck(
    pool: &[&'static dyn Role],
    target: usize,
    max_evil_threats: usize,
) -> Vec<&'static dyn Role> {
    let mut bundles: Vec<Vec<&'static dyn Role>> = Vec::new();
    let mut pending_pair: Option<&'static dyn Role> = None;
    let mut pending_groob_zerb: Option<&'static dyn Role> = None;

    for &role in pool {
        if role.requires_pair_in_deck() {
            if let Some(prev) = pending_pair {
                if prev.id() == role.id() {
                    bundles.push(vec![prev, role]);
                    pending_pair = None;
                    continue;
                }
            }
            pending_pair = Some(role);
        } else if matches!(role.id(), "groob" | "zerb") {
            if let Some(prev) = pending_groob_zerb {
                if prev.id() != role.id() {
                    bundles.push(vec![prev, role]);
                    pending_groob_zerb = None;
                    continue;
                }
            }
            pending_groob_zerb = Some(role);
        } else {
            bundles.push(vec![role]);
        }
    }

    for _attempt in 0..8 {
        fastrand::shuffle(&mut bundles);
        let mut deck: Vec<&'static dyn Role> = Vec::with_capacity(target);
        let mut threats = 0usize;

        for bundle in &bundles {
            if deck.len() + bundle.len() > target {
                continue;
            }
            let bundle_threats = bundle.iter().filter(|&&r| is_evil_threat_role(r)).count();
            if threats + bundle_threats > max_evil_threats {
                continue;
            }
            deck.extend_from_slice(bundle);
            threats += bundle_threats;
            if deck.len() == target {
                break;
            }
        }

        while deck.len() < target {
            deck.push(&VILLAGER);
        }

        let pool_has_threats = pool.iter().any(|&r| is_evil_threat_role(r));
        if !pool_has_threats || threats >= 1 {
            return deck;
        }
    }

    let mut fallback = Vec::with_capacity(target);
    let mut threats = 0usize;
    for bundle in &bundles {
        if fallback.len() + bundle.len() <= target {
            let bt = bundle.iter().filter(|&&r| is_evil_threat_role(r)).count();
            if threats + bt <= max_evil_threats {
                fallback.extend_from_slice(bundle);
                threats += bt;
            }
        }
    }
    while fallback.len() < target {
        fallback.push(&VILLAGER);
    }
    fallback
}

fn normalize_scenario_key(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .replace('ä', "a")
        .replace('ö', "o")
        .replace('ü', "u")
        .replace('ß', "ss")
        .replace(['é', 'è', 'ê'], "e")
        .replace(['à', 'â'], "a")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Iterates all 59 preset scenarios ordered by [`ScenarioPack::ALL`].
pub fn all_scenarios() -> impl Iterator<Item = &'static Scenario> {
    ScenarioPack::ALL
        .into_iter()
        .flat_map(|p| p.scenarios().iter())
}

pub fn scenario_count() -> usize {
    ScenarioPack::ALL.into_iter().map(|p| p.scenarios().len()).sum()
}

/// Returns all preset scenarios belonging to `pack` in their canonical manual/difficulty order.
pub fn scenarios_in_pack(pack: ScenarioPack) -> &'static [Scenario] {
    pack.scenarios()
}

/// Returns all preset scenarios playable at `player_count`, ordered by [`ScenarioPack::ALL`].
pub fn scenarios_for_player_count(player_count: usize) -> Vec<&'static Scenario> {
    all_scenarios()
        .filter(|s| s.supports_player_count(player_count))
        .collect()
}

/// Resolves a scenario by exact ID (`rb_base_payback`), stripped ID (`payback`), alias, or display name.
pub fn find_scenario(query: &str) -> Option<&'static Scenario> {
    let key = normalize_scenario_key(query);
    if key.is_empty() {
        return None;
    }

    for s in all_scenarios() {
        if normalize_scenario_key(s.id) == key
            || s.aliases.iter().any(|a| normalize_scenario_key(a) == key)
        {
            return Some(s);
        }
    }

    for s in all_scenarios() {
        if normalize_scenario_key(s.name) == key {
            return Some(s);
        }
        if let Some((before_paren, _)) = s.name.split_once('(') {
            if normalize_scenario_key(before_paren) == key {
                return Some(s);
            }
        }
    }

    for s in all_scenarios() {
        for prefix in [
            "rb_base_",
            "mg_combined_",
            "mg_",
            "vd_vollmond_",
            "vd_mg_",
            "vd_all3_",
            "vd_",
            "eb_vampires_",
            "eb_",
            "alien_",
            "sv_",
        ] {
            if let Some(rest) = s.id.strip_prefix(prefix) {
                if normalize_scenario_key(rest) == key {
                    return Some(s);
                }
            }
        }
    }

    None
}

/// Parses optional `<Np>` token like `"5p"` or `"7"` into `Some(5)` / `Some(7)` (`3..=10`).
pub fn parse_player_count_filter(token: &str) -> Option<usize> {
    let clean = token.trim().to_ascii_lowercase();
    let digits = clean.strip_suffix('p').unwrap_or(&clean);
    let n = digits.parse::<usize>().ok()?;
    (3..=10).contains(&n).then_some(n)
}

#[cfg(test)]
#[path = "scenarios_test.rs"]
mod test;

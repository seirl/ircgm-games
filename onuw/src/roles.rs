//! Static registry and fuzzy lookup for all 80 implemented [`Role`]s across Base + 4 Expansions.
//!
//! Submodules map 1-to-1 to the five game folders under `src/roles/` (with nothing at the root of `src/roles/`).

pub mod alien;
pub mod base;
pub mod daybreak;
pub mod super_villains;
pub mod vampire;

use crate::role::{CopierKind, GameSet, NightFaction, Phase, Role, WakeStep};

use alien::{
    alien::{ALIEN, SYNTHETIC_ALIEN},
    blob::BLOB,
    body_snatcher::BODY_SNATCHER,
    cow::COW,
    empath::EMPATH,
    exposer::EXPOSER,
    groob_zerb::{GROOB, ZERB},
    leader::LEADER,
    mortician::MORTICIAN,
    nostradamus::NOSTRADAMUS,
    oracle::ORACLE,
    psychic::PSYCHIC,
    rascal::RASCAL,
};
use base::{
    doppelganger::DOPPELGANGER, drunk::DRUNK, hunter::HUNTER, insomniac::INSOMNIAC, mason::MASON,
    minion::MINION, robber::ROBBER, seer::SEER, tanner::TANNER, troublemaker::TROUBLEMAKER,
    villager::VILLAGER, werewolf::WEREWOLF,
};
use daybreak::{
    alpha_wolf::ALPHA_WOLF, apprentice_seer::APPRENTICE_SEER, aura_seer::AURA_SEER,
    bodyguard::BODYGUARD, curator::CURATOR, cursed::CURSED, dream_wolf::DREAM_WOLF,
    mystic_wolf::MYSTIC_WOLF, paranormal_investigator::PARANORMAL_INVESTIGATOR, prince::PRINCE,
    revealer::REVEALER, sentinel::SENTINEL, village_idiot::VILLAGE_IDIOT, witch::WITCH,
};
use super_villains::{
    annoying_lad::ANNOYING_LAD,
    defender_er::DEFENDER_ER,
    detector::DETECTOR,
    dr_peeker::DR_PEEKER,
    evilometer::EVILOMETER,
    family_man::FAMILY_MAN,
    flipper::FLIPPER,
    henchman::HENCHMAN,
    innocent_bystander::INNOCENT_BYSTANDER,
    mad_scientist_intern::{INTERN, MAD_SCIENTIST},
    mirror_man::MIRROR_MAN,
    rapscallion::RAPSCALLION,
    ricochet_rhino::RICOCHET_RHINO,
    role_retriever::ROLE_RETRIEVER,
    self_awareness_girl::SELF_AWARENESS_GIRL,
    switcheroo::SWITCHEROO,
    temptress::TEMPTRESS,
    the_sponge::THE_SPONGE,
    voodoo_lou::VOODOO_LOU,
    windy_wendy::WINDY_WENDY,
};
use vampire::{
    apprentice_tanner::APPRENTICE_TANNER,
    assassin::{APPRENTICE_ASSASSIN, ASSASSIN},
    beholder::BEHOLDER,
    copycat::COPYCAT,
    count::COUNT,
    cupid::CUPID,
    diseased::DISEASED,
    gremlin::GREMLIN,
    instigator::INSTIGATOR,
    marksman::MARKSMAN,
    pickpocket::PICKPOCKET,
    priest::PRIEST,
    renfield::RENFIELD,
    squire::SQUIRE,
    the_master::THE_MASTER,
    thing::THING,
    vampire::VAMPIRE,
};

/// Complete registry of all 80 implemented [`Role`]s across Base + 4 Expansions.
pub static ALL_ROLES: &[&'static dyn Role] = &[
    // Base (12)
    &DOPPELGANGER,
    &WEREWOLF,
    &MINION,
    &MASON,
    &SEER,
    &ROBBER,
    &TROUBLEMAKER,
    &DRUNK,
    &INSOMNIAC,
    &TANNER,
    &HUNTER,
    &VILLAGER,
    // Daybreak + Bonus Pack 1 (14)
    &SENTINEL,
    &ALPHA_WOLF,
    &MYSTIC_WOLF,
    &DREAM_WOLF,
    &APPRENTICE_SEER,
    &PARANORMAL_INVESTIGATOR,
    &WITCH,
    &VILLAGE_IDIOT,
    &CURATOR,
    &REVEALER,
    &AURA_SEER,
    &BODYGUARD,
    &PRINCE,
    &CURSED,
    // Vampire + Bonus Pack 2 (18)
    &COPYCAT,
    &VAMPIRE,
    &THE_MASTER,
    &COUNT,
    &RENFIELD,
    &CUPID,
    &DISEASED,
    &INSTIGATOR,
    &ASSASSIN,
    &APPRENTICE_ASSASSIN,
    &PRIEST,
    &PICKPOCKET,
    &GREMLIN,
    &THING,
    &MARKSMAN,
    &APPRENTICE_TANNER,
    &SQUIRE,
    &BEHOLDER,
    // Alien + Bonus Pack 3 (15)
    &ALIEN,
    &SYNTHETIC_ALIEN,
    &GROOB,
    &ZERB,
    &BODY_SNATCHER,
    &LEADER,
    &ORACLE,
    &NOSTRADAMUS,
    &PSYCHIC,
    &RASCAL,
    &EXPOSER,
    &EMPATH,
    &COW,
    &BLOB,
    &MORTICIAN,
    // Super Villains + Bonus Pack 4 (21)
    &MIRROR_MAN,
    &HENCHMAN,
    &TEMPTRESS,
    &DR_PEEKER,
    &RAPSCALLION,
    &EVILOMETER,
    &ANNOYING_LAD,
    &DETECTOR,
    &ROLE_RETRIEVER,
    &VOODOO_LOU,
    &SWITCHEROO,
    &SELF_AWARENESS_GIRL,
    &FLIPPER,
    &INNOCENT_BYSTANDER,
    &MAD_SCIENTIST,
    &INTERN,
    &FAMILY_MAN,
    &WINDY_WENDY,
    &DEFENDER_ER,
    &THE_SPONGE,
    &RICOCHET_RHINO,
];

fn normalize_role_key(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .replace('ä', "a")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Resolves a role by ID, display name, or alias (case-, punctuation-, and umlaut-insensitive).
pub fn find_role(query: &str) -> Option<&'static dyn Role> {
    let key = normalize_role_key(query);
    if key.is_empty() {
        return None;
    }
    for &role in ALL_ROLES {
        if normalize_role_key(role.id()) == key || normalize_role_key(role.name()) == key {
            return Some(role);
        }
        for &alias in role.aliases() {
            if normalize_role_key(alias) == key {
                return Some(role);
            }
        }
    }
    None
}

enum WakeSlot {
    Role(&'static dyn Role, &'static str),
    Pack(NightFaction, &'static str),
    Pair(&'static str, &'static str),
    DoppelgangerNight,
}

impl WakeSlot {
    fn matches_step(&self, step: &WakeStep) -> bool {
        match self {
            Self::Role(r, _) => step.id() == r.id(),
            Self::Pack(f, _) => step.id() == f.step_id(),
            Self::Pair(id, _) => step.id() == *id,
            Self::DoppelgangerNight => false,
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::Role(_, c) | Self::Pack(_, c) | Self::Pair(_, c) => c,
            Self::DoppelgangerNight => "1",
        }
    }
}

/// Single canonical rulebook Dusk & Night waking sequence across Base + all 4 Expansions & Bonus Packs.
///
/// Individual role modules never specify order numbers; slice index **is** the execution order,
/// with `Doppelgänger` moving from `–7` (when Dusk roles are in play) to `#1` (`DoppelgangerNight`,
/// right after `#0 Sentinel`) in Night-only games.
static WAKE_TABLE: &[WakeSlot] = &[
    // ── Pre-Dusk / Early Copiers & Oracle (–9 .. –7) ─────────────────────
    WakeSlot::Role(&ORACLE, "-9"),
    WakeSlot::Role(&COPYCAT, "-8"),
    WakeSlot::Role(&MIRROR_MAN, "-8-A"),
    WakeSlot::Role(&DOPPELGANGER, "-7"),

    // ── Dusk Roles (–6 .. –1-B) ──────────────────────────────────────────
    WakeSlot::Pack(NightFaction::Vampire, "-6"),
    WakeSlot::Role(&COUNT, "-6-B"),
    WakeSlot::Role(&RENFIELD, "-6-C"),
    WakeSlot::Role(&DISEASED, "-5"),
    WakeSlot::Role(&CUPID, "-4"),
    WakeSlot::Role(&INSTIGATOR, "-3"),
    WakeSlot::Role(&PRIEST, "-2"),
    WakeSlot::Role(&ASSASSIN, "-1"),
    WakeSlot::Role(&APPRENTICE_ASSASSIN, "-1-B"),

    // ── Night Start & Monsters (0 .. 2-C) ────────────────────────────────
    WakeSlot::Role(&SENTINEL, "0"),
    WakeSlot::DoppelgangerNight,
    WakeSlot::Pack(NightFaction::Alien, "1-A"),
    WakeSlot::Role(&COW, "1-C"),
    WakeSlot::Pair("pair:groob_zerb", "1-D"),
    WakeSlot::Role(&BODY_SNATCHER, "1-F"),
    WakeSlot::Pack(NightFaction::SuperVillain, "1-SV"),
    WakeSlot::Role(&TEMPTRESS, "1-SV-B"),
    WakeSlot::Role(&DR_PEEKER, "1-SV-C"),
    WakeSlot::Role(&RAPSCALLION, "1-SV-D"),
    WakeSlot::Role(&EVILOMETER, "1-SV-E"),
    WakeSlot::Pack(NightFaction::Werewolf, "2"),
    WakeSlot::Role(&ALPHA_WOLF, "2-B"),
    WakeSlot::Role(&MYSTIC_WOLF, "2-C"),

    // ── Minions & Support (3 .. 3-E) ─────────────────────────────────────
    WakeSlot::Role(&MINION, "3"),
    WakeSlot::Role(&APPRENTICE_TANNER, "3-B"),
    WakeSlot::Role(&LEADER, "3-C"),
    WakeSlot::Role(&INTERN, "3-E"),

    // ── Masons & Bumpers (4 .. 4-C) ──────────────────────────────────────
    WakeSlot::Role(&MASON, "4"),
    WakeSlot::Role(&THING, "4-B"),
    WakeSlot::Role(&ANNOYING_LAD, "4-C"),

    // ── Seers & Investigators (5 .. 5-H) ─────────────────────────────────
    WakeSlot::Role(&SEER, "5"),
    WakeSlot::Role(&APPRENTICE_SEER, "5-B"),
    WakeSlot::Role(&PARANORMAL_INVESTIGATOR, "5-C"),
    WakeSlot::Role(&MARKSMAN, "5-D"),
    WakeSlot::Role(&NOSTRADAMUS, "5-E"),
    WakeSlot::Role(&PSYCHIC, "5-G"),
    WakeSlot::Role(&DETECTOR, "5-H"),

    // ── Single-Target Swappers & Mark Stealers (6 .. 6-E) ────────────────
    WakeSlot::Role(&ROBBER, "6"),
    WakeSlot::Role(&WITCH, "6-B"),
    WakeSlot::Role(&PICKPOCKET, "6-C"),
    WakeSlot::Role(&ROLE_RETRIEVER, "6-D"),
    WakeSlot::Role(&VOODOO_LOU, "6-E"),

    // ── Multi-Card Manipulators & Aura Seer (7 .. 7-G) ───────────────────
    WakeSlot::Role(&TROUBLEMAKER, "7"),
    WakeSlot::Role(&VILLAGE_IDIOT, "7-B"),
    WakeSlot::Role(&AURA_SEER, "7-C"),
    WakeSlot::Role(&GREMLIN, "7-D"),
    WakeSlot::Role(&RASCAL, "7-F"),
    WakeSlot::Role(&SWITCHEROO, "7-G"),

    // ── Drunk (8) ────────────────────────────────────────────────────────
    WakeSlot::Role(&DRUNK, "8"),

    // ── Late-Night Observers & Family Man (9 .. 9-Z) ─────────────────────
    WakeSlot::Role(&INSOMNIAC, "9"),
    WakeSlot::Role(&SELF_AWARENESS_GIRL, "9-B"),
    WakeSlot::Role(&SQUIRE, "9-C"),
    WakeSlot::Role(&FAMILY_MAN, "9-D"),
    WakeSlot::Role(&BEHOLDER, "9-Z"),

    // ── Card Flippers & Empath (10 .. 10-E) ──────────────────────────────
    WakeSlot::Role(&REVEALER, "10"),
    WakeSlot::Role(&EXPOSER, "10-B"),
    WakeSlot::Role(&FLIPPER, "10-C"),
    WakeSlot::Role(&EMPATH, "10-E"),

    // ── Artifacts & End-of-Night Aliens (11 .. 13-A) ─────────────────────
    WakeSlot::Role(&CURATOR, "11"),
    WakeSlot::Role(&BLOB, "13"),
    WakeSlot::Role(&MORTICIAN, "13-A"),
];

fn raw_step_rank(step: &WakeStep) -> usize {
    WAKE_TABLE
        .iter()
        .position(|slot| slot.matches_step(step))
        .unwrap_or_else(|| panic!("WakeStep `{}` is missing from WAKE_TABLE in roles.rs", step.id()))
}

fn night_boundary_rank() -> usize {
    WAKE_TABLE
        .iter()
        .position(|slot| matches!(slot, WakeSlot::Role(r, _) if r.id() == SENTINEL.info().id))
        .expect("Sentinel in WAKE_TABLE")
}

pub(crate) fn dusk_immediate_rank() -> usize {
    WAKE_TABLE
        .iter()
        .position(|slot| matches!(slot, WakeSlot::Role(r, _) if r.id() == DOPPELGANGER.info().id))
        .expect("Doppelganger in WAKE_TABLE")
}

pub(crate) fn night_immediate_rank() -> usize {
    WAKE_TABLE
        .iter()
        .position(|slot| matches!(slot, WakeSlot::DoppelgangerNight))
        .expect("DoppelgangerNight in WAKE_TABLE")
}

pub(crate) fn step_exec_rank(step: &WakeStep, has_dusk: bool) -> usize {
    if step.copier == Some(CopierKind::Player) && !has_dusk {
        night_immediate_rank()
    } else {
        raw_step_rank(step)
    }
}

pub(crate) fn is_night_step(step: &WakeStep, has_dusk: bool) -> bool {
    step_exec_rank(step, has_dusk) >= night_boundary_rank()
}

pub(crate) fn step_wake_badge(step: &WakeStep) -> String {
    if step.copier == Some(CopierKind::Player) {
        return "Dusk #-7 / Night #1".to_owned();
    }
    let idx = raw_step_rank(step);
    let code = WAKE_TABLE[idx].code();
    match step.phase {
        Phase::Dusk => format!("Dusk #{code}"),
        Phase::Night if idx >= night_boundary_rank() => format!("Night #{code}"),
        Phase::Night => format!("#{code}"),
    }
}

pub(crate) fn wakes_after_center_copy(
    copier: &'static dyn Role,
    copied: &'static dyn Role,
) -> bool {
    let Some(copier_step) = copier
        .steps()
        .iter()
        .find(|st| st.copier == Some(CopierKind::Center))
    else {
        return false;
    };
    let copier_rank = raw_step_rank(copier_step);
    !copied.is_center_copier()
        && copied.steps().iter().any(|st| raw_step_rank(st) > copier_rank)
}

pub(crate) fn wakes_after_player_copy(copied: &'static dyn Role, has_dusk: bool) -> bool {
    if copied.immediate_copy_step().is_some() {
        return false;
    }
    if copied.has_after_player_copier_step() {
        return true;
    }
    let cutoff = if has_dusk {
        dusk_immediate_rank()
    } else {
        night_immediate_rank()
    };
    copied
        .steps()
        .iter()
        .any(|st| st.copier != Some(CopierKind::Player) && step_exec_rank(st, has_dusk) > cutoff)
}

/// Returns all roles belonging to `game`, in canonical box order.
pub fn roles_in_game(game: GameSet) -> Vec<&'static dyn Role> {
    ALL_ROLES
        .iter()
        .copied()
        .filter(|r| r.game() == game)
        .collect()
}

#[cfg(test)]
#[path = "roles_test.rs"]
mod test;

//! Static registry and fuzzy lookup for all 60 implemented [`Role`]s across Base + 4 Expansions.
//!
//! Submodules map 1-to-1 to the five game folders under `src/roles/` (with nothing at the root of `src/roles/`).

pub mod alien;
pub mod base;
pub mod daybreak;
pub mod super_villains;
pub mod vampire;

use crate::role::{GameSet, Role};

use alien::{
    alien::{ALIEN, SYNTHETIC_ALIEN},
    blob::BLOB,
    cow::COW,
    groob_zerb::{GROOB, ZERB},
    leader::LEADER,
    mortician::MORTICIAN,
    oracle::ORACLE,
    psychic::PSYCHIC,
};
use base::{
    doppelganger::DOPPELGANGER, drunk::DRUNK, hunter::HUNTER, insomniac::INSOMNIAC, mason::MASON,
    minion::MINION, robber::ROBBER, seer::SEER, tanner::TANNER, troublemaker::TROUBLEMAKER,
    villager::VILLAGER, werewolf::WEREWOLF,
};
use daybreak::{
    alpha_wolf::ALPHA_WOLF, apprentice_seer::APPRENTICE_SEER, aura_seer::AURA_SEER,
    bodyguard::BODYGUARD, curator::CURATOR, dream_wolf::DREAM_WOLF, mystic_wolf::MYSTIC_WOLF,
    paranormal_investigator::PARANORMAL_INVESTIGATOR, prince::PRINCE, revealer::REVEALER,
    sentinel::SENTINEL, village_idiot::VILLAGE_IDIOT, witch::WITCH,
};
use super_villains::{
    annoying_lad::ANNOYING_LAD,
    dr_peeker::DR_PEEKER,
    evilometer::EVILOMETER,
    henchman::HENCHMAN,
    mad_scientist_intern::{INTERN, MAD_SCIENTIST},
    rapscallion::RAPSCALLION,
    rogue::ROGUE,
    role_retriever::ROLE_RETRIEVER,
    temptress::TEMPTRESS,
};
use vampire::{
    assassin::{APPRENTICE_ASSASSIN, ASSASSIN},
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
    the_master::THE_MASTER,
    thing::THING,
    vampire::VAMPIRE,
};

/// Complete registry of all 60 implemented [`Role`]s across Base + 4 Expansions.
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
    // Daybreak (13)
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
    // Vampire (15)
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
    // Alien (10)
    &ALIEN,
    &SYNTHETIC_ALIEN,
    &GROOB,
    &ZERB,
    &LEADER,
    &ORACLE,
    &PSYCHIC,
    &COW,
    &BLOB,
    &MORTICIAN,
    // Super Villains (10)
    &HENCHMAN,
    &TEMPTRESS,
    &DR_PEEKER,
    &RAPSCALLION,
    &EVILOMETER,
    &ROLE_RETRIEVER,
    &ANNOYING_LAD,
    &MAD_SCIENTIST,
    &INTERN,
    &ROGUE,
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

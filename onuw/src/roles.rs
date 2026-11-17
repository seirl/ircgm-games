//! Static registry and fuzzy lookup for all 80 implemented [`Role`]s across Base + 4 Expansions.
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

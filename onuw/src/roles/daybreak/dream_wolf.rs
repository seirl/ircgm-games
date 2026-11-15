use crate::role::{GameSet, Role, RoleInfo, RoleTag, Team};

pub struct DreamWolf;
pub static DREAM_WOLF: DreamWolf = DreamWolf;

impl Role for DreamWolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "dream_wolf",
            "Dream Wolf",
            GameSet::Daybreak,
            Team::Werewolf,
            "You sleep through the night (do NOT wake with other Werewolves), though the Minion still sees your thumb!",
        )
        .aliases(&["dw"])
        .tags(&[RoleTag::DreamWolfThumb])
    }
}

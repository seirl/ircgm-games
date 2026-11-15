use crate::role::{GameSet, Role, RoleInfo, Team};

pub struct Villager;
pub static VILLAGER: Villager = Villager;

impl Role for Villager {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "villager",
            "Villager",
            GameSet::Base,
            Team::Village,
            "Sleep at night. Deduce who holds an Evil threat card and vote them out!",
        )
    }
}

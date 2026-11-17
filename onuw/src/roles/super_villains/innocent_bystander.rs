use crate::role::{GameSet, Role, RoleInfo, Team};

pub struct InnocentBystander;
pub static INNOCENT_BYSTANDER: InnocentBystander = InnocentBystander;

impl Role for InnocentBystander {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "innocent_bystander",
            "Innocent Bystander",
            GameSet::SuperVillains,
            Team::Village,
            "Sleep at night. You have no powers (and Flipper/Revealer flips your card back face-down!), and you win if the Hero/Village team wins!",
        )
        .aliases(&["bystander", "ib", "2nd innocent bystander"])
        .flips_back_on_reveal()
    }
}

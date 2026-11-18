use crate::role::{GameSet, Role, RoleInfo, SpecialVictory, Team, VictoryCtx};

pub struct InnocentBystander;
pub static INNOCENT_BYSTANDER: InnocentBystander = InnocentBystander;

impl Role for InnocentBystander {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "innocent_bystander",
            "Innocent Bystander",
            GameSet::SuperVillains,
            Team::Independent,
            "Sleep at night. You are not on any team (so Flipper/Revealer flips your card back face-down!), and you win if the Hero/Village team wins!",
        )
        .aliases(&["bystander", "ib", "2nd innocent bystander"])
    }

    fn special_victory(&self, _seat: usize, _ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        Some(SpecialVictory::with_team(Team::Village))
    }
}

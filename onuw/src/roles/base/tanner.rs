use crate::role::{GameSet, Role, RoleInfo, SpecialVictory, Team, VictoryCtx};

pub struct Tanner;
pub static TANNER: Tanner = Tanner;

impl Role for Tanner {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "tanner",
            "Tanner",
            GameSet::Base,
            Team::Tanner,
            "You hate your job. You ONLY win if the village kills you!",
        )
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        // ONUW Tanner rule: if Tanner dies and no Monster dies, Werewolf & Vampire Teams lose;
        // if a Monster ALSO dies, Village still wins alongside Tanner!
        ctx.is_dead(seat)
            .then_some(SpecialVictory::independent_blocking_monsters())
    }
}

#[cfg(test)]
#[path = "tanner_test.rs"]
mod test;

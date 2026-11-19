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
        // Rule (ONUW — Tanner): "The Tanner only wins if he dies. If the
        // Tanner dies and no Werewolves die, the Werewolves do not win. If the
        // Tanner dies and a Werewolf also dies, the village team wins too."
        ctx.is_dead(seat)
            .then_some(SpecialVictory::independent_blocking_monsters())
    }
}

#[cfg(test)]
#[path = "tanner_test.rs"]
mod test;

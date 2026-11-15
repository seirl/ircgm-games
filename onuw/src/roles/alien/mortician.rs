use crate::role::{GameSet, Role, RoleInfo, SpecialVictory, Team, VictoryCtx};

pub struct Mortician;
pub static MORTICIAN: Mortician = Mortician;

impl Role for Mortician {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mortician",
            "Mortician",
            GameSet::Alien,
            Team::Independent,
            "Sleep at night. You win ONLY if you survive AND at least one of your immediate neighbors dies!",
        )
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if ctx.is_dead(seat) {
            return None;
        }
        let (left, right) = ctx.roster.neighbors(seat);
        (ctx.is_dead(left) || ctx.is_dead(right)).then_some(SpecialVictory::independent())
    }
}

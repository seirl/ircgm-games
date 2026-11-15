use ircgm::prelude::*;

use crate::board::Roster;
use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, SpecialVictory, Team,
    VictoryCtx, Wake,
};

pub struct MadScientist;
pub static MAD_SCIENTIST: MadScientist = MadScientist;

pub struct Intern;
pub static INTERN: Intern = Intern;

impl Role for MadScientist {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mad_scientist",
            "Mad Scientist",
            GameSet::SuperVillains,
            Team::Independent,
            "Your invention requires a jolt—you ONLY win if YOU are killed!",
        )
        .aliases(&["ms", "scientist"])
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        ctx.is_dead(seat)
            .then_some(SpecialVictory::independent_blocking(Team::MONSTER_TEAMS))
    }
}

impl Role for Intern {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "intern",
            "Intern",
            GameSet::SuperVillains,
            Team::Independent,
            "Wake to see who the Mad Scientist is. You win if the Mad Scientist dies (or if no Mad Scientist is in play and YOU die)!",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has(&MAD_SCIENTIST) {
            self.info().summary.to_owned()
        } else {
            "You ONLY win if YOU get killed!".to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(132, Self::wake_group))
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let scientists = ctx.find_seats(&MAD_SCIENTIST);
        let won = if scientists.is_empty() {
            ctx.is_dead(seat)
        } else {
            scientists.into_iter().any(|m| ctx.is_dead(m))
        };
        won.then_some(SpecialVictory::independent())
    }
}

impl Intern {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let scientists = ctx.seats_with_role(&MAD_SCIENTIST);
            for &s in &active_seats {
                if scientists.is_empty() {
                    ctx.dm(
                        s,
                        "🥽 Intern: No Mad Scientist is in play — get YOURSELF killed to take over the lab and win!",
                    );
                } else {
                    ctx.dm(
                        s,
                        irc_format!(
                            "🥽 Intern: The Mad Scientist is {} — help them get killed so you both win!",
                            ctx.roster.format_indices(&scientists).bold().orange(),
                        ),
                    );
                }
            }
            Ok(GroupEffect::new(move |_board, rctx| {
                let interns = rctx.roster.format_indices(&active_seats);
                let styled = INTERN.info().styled_name();
                if scientists.is_empty() {
                    rctx.record(irc_format!(
                        "🥽 {styled} ({}) saw no Mad Scientist in play.",
                        interns.bold(),
                    ));
                } else {
                    let sci_names = rctx.roster.format_indices(&scientists);
                    rctx.record(irc_format!(
                        "🥽 {styled} ({}) spotted Mad Scientist {}.",
                        interns.bold(),
                        sci_names.bold().orange(),
                    ));
                }
            }))
        })
    }
}

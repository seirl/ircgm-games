use ircgm::prelude::*;

use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, RoleTag, SpecialVictory, Team,
    VictoryCtx, Wake,
};

pub struct Groob;
pub static GROOB: Groob = Groob;

pub struct Zerb;
pub static ZERB: Zerb = Zerb;

impl Role for Groob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "groob",
            "Groob",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: you appear Alien, know who Zerb is, and win ONLY if Zerb dies!",
        )
        .tags(&[RoleTag::AlienPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(126, Self::wake_group))
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let zerbs = ctx.find_seats(&ZERB);
        (!zerbs.is_empty() && zerbs.into_iter().any(|z| ctx.is_dead(z)))
            .then_some(SpecialVictory::independent())
    }
}

impl Groob {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let zerb_seats = ctx.seats_with_role(&ZERB);
            let zerb_info = if zerb_seats.is_empty() {
                "in the center".to_owned()
            } else {
                ctx.roster.format_indices(&zerb_seats)
            };
            for &s in &active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 Groob: Your mortal nemesis Zerb is {} — get Zerb killed to win!",
                        zerb_info.as_str().bold().red(),
                    ),
                );
            }
            Ok(GroupEffect::new(move |_board, rctx| {
                let names = rctx.roster.format_indices(&active_seats);
                rctx.record(irc_format!(
                    "👾 {} ({}) spotted nemesis Zerb ({}).",
                    GROOB.info().styled_name(),
                    names.bold(),
                    zerb_info.as_str().bold(),
                ));
            }))
        })
    }
}

impl Role for Zerb {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "zerb",
            "Zerb",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: you appear Alien, know who Groob is, and win ONLY if Groob dies!",
        )
        .tags(&[RoleTag::AlienPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(127, Self::wake_group))
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let groobs = ctx.find_seats(&GROOB);
        (!groobs.is_empty() && groobs.into_iter().any(|g| ctx.is_dead(g)))
            .then_some(SpecialVictory::independent())
    }
}

impl Zerb {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let groob_seats = ctx.seats_with_role(&GROOB);
            let groob_info = if groob_seats.is_empty() {
                "in the center".to_owned()
            } else {
                ctx.roster.format_indices(&groob_seats)
            };
            for &s in &active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 Zerb: Your mortal nemesis Groob is {} — get Groob killed to win!",
                        groob_info.as_str().bold().red(),
                    ),
                );
            }
            Ok(GroupEffect::new(move |_board, rctx| {
                let names = rctx.roster.format_indices(&active_seats);
                rctx.record(irc_format!(
                    "👾 {} ({}) spotted nemesis Groob ({}).",
                    ZERB.info().styled_name(),
                    names.bold(),
                    groob_info.as_str().bold(),
                ));
            }))
        })
    }
}

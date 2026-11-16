use ircgm::prelude::*;

use crate::marks::Mark;
use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, RoleTag, SpecialVictory, Team,
    VictoryCtx, Wake,
};

pub struct Groob;
pub static GROOB: Groob = Groob;

pub struct Zerb;
pub static ZERB: Zerb = Zerb;

fn effective_seats_for(ctx: &VictoryCtx<'_>, role_id: &str) -> Vec<usize> {
    (0..ctx.roster.len())
        .filter(|&i| {
            !ctx.board.has_role_override_artifact(i)
                && ctx.board.marks[i] != Mark::Vampire
                && ctx.board.players[i].effective_role().id() == role_id
        })
        .collect()
}

impl Role for Groob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "groob",
            "Groob",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: wake with the Aliens; if Zerb is in play, win ONLY if you survive and Zerb dies (otherwise win with the Aliens)!",
        )
        .tags(&[RoleTag::AlienPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(114, Self::wake_group))
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !ctx.board.both_groob_and_zerb_in_play() || ctx.is_dead(seat) {
            return None;
        }
        let zerbs = effective_seats_for(ctx, "zerb");
        zerbs
            .into_iter()
            .any(|z| ctx.is_dead(z))
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
                for &s in &active_seats {
                    ctx.dm(
                        s,
                        "👾 Groob: Zerb is in the center — you are on the regular Alien team tonight!",
                    );
                }
                "in the center".to_owned()
            } else {
                let names = ctx.roster.format_indices(&zerb_seats);
                for &s in &active_seats {
                    ctx.dm(
                        s,
                        irc_format!(
                            "👾 Groob: Your mortal nemesis Zerb is {} — survive and get Zerb killed to win!",
                            names.as_str().bold().red(),
                        ),
                    );
                }
                names
            };
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
            "Rival Alien: wake with the Aliens; if Groob is in play, win ONLY if you survive and Groob dies (otherwise win with the Aliens)!",
        )
        .tags(&[RoleTag::AlienPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(115, Self::wake_group))
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !ctx.board.both_groob_and_zerb_in_play() || ctx.is_dead(seat) {
            return None;
        }
        let groobs = effective_seats_for(ctx, "groob");
        groobs
            .into_iter()
            .any(|g| ctx.is_dead(g))
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
                for &s in &active_seats {
                    ctx.dm(
                        s,
                        "👾 Zerb: Groob is in the center — you are on the regular Alien team tonight!",
                    );
                }
                "in the center".to_owned()
            } else {
                let names = ctx.roster.format_indices(&groob_seats);
                for &s in &active_seats {
                    ctx.dm(
                        s,
                        irc_format!(
                            "👾 Zerb: Your mortal nemesis Groob is {} — survive and get Groob killed to win!",
                            names.as_str().bold().red(),
                        ),
                    );
                }
                names
            };
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

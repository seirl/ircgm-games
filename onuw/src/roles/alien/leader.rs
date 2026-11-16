use ircgm::prelude::*;

use super::groob_zerb::{GROOB, ZERB};
use crate::marks::Mark;
use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, RoleTag, SpecialVictory, Team,
    VictoryCtx, Wake,
};

pub struct Leader;
pub static LEADER: Leader = Leader;

impl Role for Leader {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "leader",
            "Leader",
            GameSet::Alien,
            Team::Village,
            "Knows who the Aliens are! WARNING: if ALL Aliens vote for you, Aliens win (or if both Groob & Zerb are in play, win ONLY if both survive)!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(135, Self::wake_group))
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if ctx.board.both_groob_and_zerb_in_play() {
            let gz_survive = (0..ctx.roster.len())
                .filter(|&i| {
                    !ctx.board.has_role_override_artifact(i)
                        && ctx.board.marks[i] != Mark::Vampire
                        && matches!(ctx.board.players[i].effective_role().id(), "groob" | "zerb")
                })
                .all(|i| !ctx.is_dead(i));
            return gz_survive.then_some(SpecialVictory::independent());
        }

        let alien_threats: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| ctx.board.is_alien_threat(i))
            .collect();

        if !alien_threats.is_empty() && alien_threats.iter().all(|&a| ctx.votes[a] == Some(seat)) {
            Some(SpecialVictory::force_team(
                Team::Alien,
                format!(
                    "🛸 All Aliens pointed at the Leader ({}) — Aliens abduct the Leader and WIN!",
                    ctx.roster.seats[seat].user.nick()
                )
                .bold()
                .yellow(),
            ))
        } else {
            None
        }
    }
}

impl Leader {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let aliens = ctx.seats_with_tag(RoleTag::AlienPack);
            let groobs = ctx.seats_with_role(&GROOB);
            let zerbs = ctx.seats_with_role(&ZERB);
            let mut gz_seats = Vec::new();
            if !groobs.is_empty() && !zerbs.is_empty() {
                gz_seats.extend(groobs);
                gz_seats.extend(zerbs);
                gz_seats.sort_unstable();
                gz_seats.dedup();
            }

            for &s in &active_seats {
                if aliens.is_empty() {
                    ctx.dm(s, "🫡 Leader: No Aliens are held by players tonight!");
                } else {
                    ctx.dm(
                        s,
                        irc_format!(
                            "🫡 Leader: The Aliens are {} — stay hidden so they don't all vote for you!",
                            ctx.roster.format_indices(&aliens).bold().red(),
                        ),
                    );
                }
                if !gz_seats.is_empty() {
                    ctx.dm(
                        s,
                        irc_format!(
                            "👾 Leader Intel: Groob & Zerb are pointing at each other ({}) — you ONLY win if NEITHER Groob nor Zerb dies!",
                            ctx.roster.format_indices(&gz_seats).bold().orange(),
                        ),
                    );
                }
            }
            Ok(GroupEffect::new(move |_board, rctx| {
                let leaders = rctx.roster.format_indices(&active_seats);
                let styled = LEADER.info().styled_name();
                if aliens.is_empty() {
                    rctx.record(irc_format!(
                        "🫡 {styled} ({}) saw no Aliens in play.",
                        leaders.bold(),
                    ));
                } else {
                    let alien_nicks = rctx.roster.format_indices(&aliens);
                    rctx.record(irc_format!(
                        "🫡 {styled} ({}) identified Aliens: {}.",
                        leaders.bold(),
                        alien_nicks.bold().red(),
                    ));
                }
            }))
        })
    }
}

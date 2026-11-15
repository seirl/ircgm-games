use ircgm::prelude::*;

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
            "Knows who the Aliens are! WARNING: if ALL Aliens point at you during the vote, the Aliens win regardless of who dies!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(135, Self::wake_group))
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let alien_threats: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| {
                let r = ctx.board.players[i].effective_role();
                r.tags().contains(&RoleTag::AlienPack) && r.is_primary_threat()
            })
            .collect();

        if !alien_threats.is_empty() && alien_threats.iter().all(|&a| ctx.votes[a] == Some(seat)) {
            Some(SpecialVictory::force_team(
                Team::Werewolf,
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

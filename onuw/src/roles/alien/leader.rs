use ircgm::prelude::*;

use super::groob_zerb::{both_groob_and_zerb_in_play, GROOB, ZERB};
use crate::board::{BoardCards, SeatSet};
use crate::role::{
    EffectiveAlignment, GameSet, GroupEffect, GroupPromptCtx, NightFaction, Role, RoleInfo,
    SpecialVictory, Team, VictoryCtx, WakeStep,
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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::group(&LEADER, 135, |seats, ctx| {
                Box::pin(Self::wake_group(seats, ctx))
            })]
        }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        if both_groob_and_zerb_in_play(board) {
            cur.team = Team::Independent;
        }
        cur
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if both_groob_and_zerb_in_play(ctx.board) {
            let gz_survive =
                (ctx.active_holders(&GROOB) | ctx.active_holders(&ZERB)).is_disjoint(ctx.dead_seats);
            return gz_survive.then_some(SpecialVictory::independent());
        }

        let alien_threats: SeatSet = ctx
            .roster
            .all_seats()
            .into_iter()
            .filter(|&i| ctx.board.is_monster_threat_for(i, Team::Alien))
            .collect();

        if !alien_threats.is_empty() && alien_threats.into_iter().all(|a| ctx.votes[a] == Some(seat))
        {
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
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let aliens = ctx.seats_with_faction(NightFaction::Alien);
        let groobs = ctx.waking_seats_with_role(&GROOB);
        let zerbs = ctx.waking_seats_with_role(&ZERB);
        let gz_seats = if !groobs.is_empty() && !zerbs.is_empty() {
            groobs | zerbs
        } else {
            SeatSet::EMPTY
        };
        let thumb_aliens = aliens - gz_seats;

        for s in active_seats {
            if aliens.is_empty() {
                ctx.dm(s, "🫡 Leader: No Aliens are held by players tonight!");
            } else if gz_seats.is_empty() {
                ctx.dm(
                    s,
                    irc_format!(
                        "🫡 Leader: The Aliens are {} — stay hidden so they don't all vote for you!",
                        ctx.roster.format_seats(aliens).bold().red(),
                    ),
                );
            } else {
                if !thumb_aliens.is_empty() {
                    ctx.dm(
                        s,
                        irc_format!(
                            "🫡 Leader: Other Aliens in play: {}.",
                            ctx.roster.format_seats(thumb_aliens).bold().red(),
                        ),
                    );
                }
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 Leader Intel: Groob & Zerb are pointing at each other ({}) — you ONLY win if NEITHER Groob nor Zerb dies!",
                        ctx.roster.format_seats(gz_seats).bold().orange(),
                    ),
                );
            }
        }
        Ok(GroupEffect::new(move |_board, rctx| {
            let leaders = rctx.roster.format_seats(active_seats);
            let styled = LEADER.info().styled_name();
            if aliens.is_empty() {
                rctx.record(irc_format!(
                    "🫡 {styled} ({}) saw no Aliens in play.",
                    leaders.bold(),
                ));
            } else {
                let alien_nicks = rctx.roster.format_seats(aliens);
                rctx.record(irc_format!(
                    "🫡 {styled} ({}) identified Aliens: {}.",
                    leaders.bold(),
                    alien_nicks.bold().red(),
                ));
            }
        }))
    }
}

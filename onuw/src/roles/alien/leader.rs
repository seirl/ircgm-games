use ircgm::prelude::*;

use super::groob_zerb::{both_groob_and_zerb_in_play, groob_zerb_pointers, GROOB, ZERB};
use crate::board::{BoardCards, BoardMut, SeatSet};
use crate::role::{
    EffectiveAlignment, GameSet, NightFaction, ResolveCtx, Role, RoleInfo, SpecialVictory, Team,
    VictoryCtx, WakeStep,
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
        const { &[WakeStep::passive(&LEADER, Self::inspect)] }
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

    fn special_victories(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Vec<SpecialVictory> {
        let mut claims = Vec::new();

        // ONUA rulebook: "the village team loses and the alien team wins if all players on the
        // alien team (including Zerb and Groob, if either or both are in the game) point to the
        // Leader, even if an alien is killed" — applies even when both Groob & Zerb are in play!
        let alien_voters: SeatSet = ctx
            .roster
            .all_seats()
            .into_iter()
            .filter(|&i| {
                ctx.board.effective_team(i) == Team::Alien
                    || ctx.board.is_monster_threat_for(i, Team::Alien)
            })
            .collect();

        if !alien_voters.is_empty() && alien_voters.into_iter().all(|a| ctx.votes[a] == seat)
        {
            claims.push(SpecialVictory::force_team(
                Team::Alien,
                format!(
                    "🛸 All Aliens pointed at the Leader ({}) — Aliens abduct the Leader and WIN!",
                    ctx.roster.seats[seat].user.nick()
                )
                .bold()
                .yellow(),
            ));
        }

        if both_groob_and_zerb_in_play(ctx.board)
            && (ctx.active_holders(&GROOB) | ctx.active_holders(&ZERB)).is_disjoint(ctx.dead_seats)
        {
            claims.push(SpecialVictory::independent());
        }

        claims
     }
}

impl Leader {
    fn inspect(
        seat: usize,
        actor_label: &str,
        _board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let aliens = ctx.seats_with_faction(NightFaction::Alien);
        let pointers = groob_zerb_pointers(
            ctx.waking_seats_with_role(&GROOB),
            ctx.waking_seats_with_role(&ZERB),
            |s| ctx.copied_from(s),
        );
        let pointing_seats: SeatSet = pointers.iter().map(|&(s, _)| s).collect();
        let thumb_aliens = aliens - pointing_seats;

        let is_clean_pair = pointers.len() == 2
            && pointers[0].1 == SeatSet::single(pointers[1].0)
            && pointers[1].1 == SeatSet::single(pointers[0].0);

        let vectors_str: String = pointers
            .iter()
            .map(|&(src, dsts)| {
                let dst_str = ctx.roster.format_seats(dsts);
                if dsts.len() >= 2 {
                    format!("{} → {dst_str} (both hands)", ctx.nick(src))
                } else {
                    format!("{} → {dst_str}", ctx.nick(src))
                }
            })
            .collect::<Vec<_>>()
            .join("; ");

        let actor_nick = ctx.nick(seat);
        if aliens.is_empty() {
            ctx.dm(seat, format!("🫡 {actor_label}: No Aliens are held by players tonight!"));
            ctx.record(irc_format!(
                "🫡 {actor_label} ({}) saw no Aliens in play.",
                actor_nick.bold(),
            ));
        } else {
            if pointers.is_empty() {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🫡 {actor_label}: The Aliens are {} — stay hidden so they don't all vote for you!",
                        ctx.roster.format_seats(aliens).bold().red(),
                    ),
                );
            } else if is_clean_pair {
                if !thumb_aliens.is_empty() {
                    ctx.dm(
                        seat,
                        irc_format!(
                            "🫡 {actor_label}: Other Aliens in play: {}.",
                            ctx.roster.format_seats(thumb_aliens).bold().red(),
                        ),
                    );
                }
                ctx.dm(
                    seat,
                    irc_format!(
                        "👾 Leader Intel: Groob & Zerb are pointing at each other ({}) — you ONLY win if NEITHER Groob nor Zerb dies!",
                        ctx.roster.format_seats(pointing_seats).bold().orange(),
                    ),
                );
            } else {
                if !thumb_aliens.is_empty() {
                    ctx.dm(
                        seat,
                        irc_format!(
                            "🫡 {actor_label}: Thumb-up Aliens: {}.",
                            ctx.roster.format_seats(thumb_aliens).bold().red(),
                        ),
                    );
                }
                ctx.dm(
                    seat,
                    irc_format!(
                        "👾 Leader Intel: Groob/Zerb pointing: {} (if both Groob & Zerb are in play, you ONLY win if ALL Groob & Zerb holders survive)!",
                        vectors_str.as_str().bold().orange(),
                    ),
                );
            }
            let alien_nicks = ctx.roster.format_seats(aliens);
            ctx.record(irc_format!(
                "🫡 {actor_label} ({}) identified Aliens: {}.",
                actor_nick.bold(),
                alien_nicks.bold().red(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "leader_test.rs"]
mod test;

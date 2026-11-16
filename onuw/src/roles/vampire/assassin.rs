use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::marks::Mark;
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag,
    SeatEffect, SpecialVictory, Team, VictoryCtx, Wake,
};

pub struct Assassin;
pub static ASSASSIN: Assassin = Assassin;

pub struct ApprenticeAssassin;
pub static APPRENTICE_ASSASSIN: ApprenticeAssassin = ApprenticeAssassin;

impl Role for Assassin {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "assassin",
            "Assassin",
            GameSet::Vampire,
            Team::Independent,
            "Place the Mark of the Assassin on any player (including yourself) in Dusk. You win ONLY if the player holding that Mark dies!",
        )
        .tags(&[RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(
            Wake::solo(44, |seat, ctx| Box::pin(Self::prompt(seat, ctx)))
                .dusk()
                .mutates(),
        )
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        (0..ctx.roster.len())
            .any(|s| ctx.board.marks[s] == Mark::Assassin && ctx.is_dead(s))
            .then_some(SpecialVictory::independent())
    }
}

impl Assassin {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let prompt = irc_format!(
            "🗡️ Assassin: Reply {} to place the Mark of the Assassin (any player, including yourself)! Players: {}",
            "<nick>".bold(),
            roster.format_players(),
        );
        let ans = ctx
            .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                Some(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🗡️ Placed {} on {}!",
                        Mark::Assassin.styled(),
                        roster.seats[idx].user.nick().bold(),
                    ),
                ),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<usize>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let target = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                ctx.roster.neighbors(seat).1
            }
        };
        board.marks[target] = Mark::Assassin;
        ctx.record(irc_format!(
            "🗡️ {actor_label} ({}) placed {} on {}.",
            ctx.nick(seat).bold(),
            Mark::Assassin.styled(),
            ctx.nick(target).bold(),
        ));
    }
}

impl Role for ApprenticeAssassin {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "apprentice_assassin",
            "Apprentice Assassin",
            GameSet::Vampire,
            Team::Independent,
            "Wake in Dusk to see the Assassin (you win if the Assassin dies!). If no Assassin is in play, YOU place the Mark of the Assassin!",
        )
        .aliases(&["aa", "appassassin"])
        .tags(&[RoleTag::PlacesMarks])
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has(&ASSASSIN) {
            self.info().summary.to_owned()
        } else {
            "Place the Mark of the Assassin on any player in Dusk. You win ONLY if the player holding that Mark dies!".to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(
            Wake::group(46, |seats, ctx| Box::pin(Self::wake_group(seats, ctx)))
                .dusk()
                .mutates(),
        )
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let won = if ctx.board.apprentice_assassin_marked {
            (0..ctx.roster.len()).any(|s| ctx.board.marks[s] == Mark::Assassin && ctx.is_dead(s))
        } else {
            let assassins = ctx.find_seats(&ASSASSIN);
            if !assassins.is_empty() {
                assassins.into_iter().any(|a| ctx.is_dead(a))
            } else {
                (0..ctx.roster.len())
                    .any(|s| ctx.board.marks[s] == Mark::Assassin && ctx.is_dead(s))
            }
        };
        won.then_some(SpecialVictory::independent())
    }
}

impl ApprenticeAssassin {
    async fn wake_group(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let assassins = ctx.seats_with_role(&ASSASSIN);
        if !assassins.is_empty() {
            let list = ctx.roster.format_indices(&assassins);
            for &s in &active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "🗡️ Apprentice Assassin: Your mentor Assassin is {} — get them killed to take over the guild and win!",
                        list.as_str().bold().orange(),
                    ),
                );
            }
            return Ok(GroupEffect::new(move |_board, rctx| {
                let names = rctx.roster.format_indices(&active_seats);
                rctx.record(irc_format!(
                    "🗡️ Apprentice Assassin ({}) identified their mentor Assassin.",
                    names.bold(),
                ));
            }));
        }

        let actor = active_seats[0];
        if active_seats.len() > 1 {
            for &s in &active_seats[1..] {
                ctx.dm(
                    s,
                    irc_format!(
                        "🗡️ Apprentice Assassin: No Assassin is in play! Fellow Apprentice {} is placing the {}!",
                        ctx.nick(actor).bold(),
                        Mark::Assassin.styled(),
                    ),
                );
            }
        }
        let roster = ctx.roster;
        let prompt = irc_format!(
            "🗡️ Apprentice Assassin: No Assassin is in play! Reply {} to place the Mark of the Assassin (any player, including yourself)! Players: {}",
            "<nick>".bold(),
            roster.format_players(),
        );
        let ans = ctx
            .ask(actor, prompt, move |m| match roster.resolve_nick(&m.text) {
                Some(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🗡️ Placed {} on {}!",
                        Mark::Assassin.styled(),
                        roster.seats[idx].user.nick().bold(),
                    ),
                ),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;
        let target = ans.ok().unwrap_or_else(|| ctx.roster.neighbors(actor).1);

        Ok(GroupEffect::new(move |board, rctx| {
            board.marks[target] = Mark::Assassin;
            board.apprentice_assassin_marked = true;
            for &s in &active_seats[1..] {
                rctx.dm(
                    s,
                    irc_format!(
                        "🗡️ Fellow Apprentice Assassin placed {} on {}!",
                        Mark::Assassin.styled(),
                        rctx.nick(target).bold(),
                    ),
                );
            }
            rctx.record(irc_format!(
                "🗡️ Apprentice Assassin ({}) placed {} on {}.",
                rctx.nick(actor).bold(),
                Mark::Assassin.styled(),
                rctx.nick(target).bold(),
            ));
        }))
    }
}

use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardCards, BoardKey, BoardMut, Roster, SeatSet};
use crate::marks::Mark;
use crate::role::{
    EffectiveAlignment, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, SpecialVictory,
    Team, VictoryCtx, WakeStep,
};

pub struct Assassin;
pub static ASSASSIN: Assassin = Assassin;

pub struct ApprenticeAssassin;
pub static APPRENTICE_ASSASSIN: ApprenticeAssassin = ApprenticeAssassin;

#[derive(Clone, Copy, Debug)]
struct AssassinDuskState {
    mentor_assassins: SeatSet,
    apprentice_placed_mark: bool,
}

impl BoardKey for AssassinDuskState {
    const ACCESS: u16 = BoardAccess::CUSTOM_1;

    fn init() -> Self {
        Self {
            mentor_assassins: SeatSet::EMPTY,
            apprentice_placed_mark: false,
        }
    }
}

impl Role for Assassin {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "assassin",
            "Assassin",
            GameSet::Vampire,
            Team::Independent,
            "Place the Mark of the Assassin on any player (including yourself) in Dusk. You win ONLY if the player holding that Mark dies!",
        )
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&ASSASSIN, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .dusk()
                .copy_on_turn()
                .writes(BoardAccess::MARKS | AssassinDuskState::ACCESS)]
        }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        if !board.marks().contains(&Mark::Assassin) {
            cur.team = Team::Village;
        }
        cur
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        ctx.dead_seats
            .into_iter()
            .any(|s| ctx.board.mark(s) == Mark::Assassin)
            .then_some(SpecialVictory::independent())
    }
}

impl Assassin {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🗡️ Assassin: Reply {} to place the Mark of the Assassin (any player, including yourself)! Players: {players_str}",
            "<nick>".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| match view.resolve_nick(&m.text) {
                Some(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🗡️ Placed {} on {}!",
                        Mark::Assassin.styled(),
                        view.roster.seats[idx].user.nick().bold(),
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
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let target = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                ctx.roster.neighbors(seat).1
            }
        };
        board.set_mark(target, Mark::Assassin);
        if ctx.copied_from(seat).is_none() {
            let mut state = board.get_key::<AssassinDuskState>();
            state.mentor_assassins.insert(seat);
            board.set_key(state);
        }
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
        .places_marks()
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has(&ASSASSIN) {
            self.info().summary.to_owned()
        } else {
            "Place the Mark of the Assassin on any player in Dusk. You win ONLY if the player holding that Mark dies!".to_owned()
        }
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&APPRENTICE_ASSASSIN, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .dusk()
                    .copy_on_turn()
                    .reads(BoardAccess::MARKS | AssassinDuskState::ACCESS)
                    .writes(BoardAccess::MARKS | AssassinDuskState::ACCESS),
            ]
        }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        if !board.marks().contains(&Mark::Assassin) {
            cur.team = Team::Village;
        }
        cur
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !ctx.board.marks().contains(&Mark::Assassin) {
            return None;
        }
        let state = ctx.board.get_key::<AssassinDuskState>();
        let won = if state.apprentice_placed_mark {
            ctx.dead_seats
                .into_iter()
                .any(|s| ctx.board.mark(s) == Mark::Assassin)
        } else {
            ctx.role_card_holders(&ASSASSIN).intersects(ctx.dead_seats)
        };
        won.then_some(SpecialVictory::independent())
    }
}

impl ApprenticeAssassin {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let mentors = ctx.get_key::<AssassinDuskState>().mentor_assassins;
        if !mentors.is_empty() {
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                let list = rctx.roster.format_seats(mentors);
                let app_nick = rctx.nick(seat);
                rctx.dm(
                    seat,
                    irc_format!(
                        "🗡️ {label}: Your mentor Assassin is {} — get them killed to take over the guild and win!",
                        list.as_str().bold().orange(),
                    ),
                );
                for a_seat in mentors {
                    rctx.dm(
                        a_seat,
                        irc_format!(
                            "🗡️ Assassin: Your {label} is {} — watch your back!",
                            app_nick.as_str().bold().orange(),
                        ),
                    );
                }
                rctx.record(irc_format!(
                    "🗡️ {label} ({}) and Assassin ({}) recognized each other.",
                    app_nick.bold(),
                    list.as_str().bold(),
                ));
            }));
        }

        let players_str = ctx.roster.format_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "🗡️ Apprentice Assassin: No Assassin is in play! Reply {} to place the Mark of the Assassin (any player, including yourself)! Players: {players_str}",
            "<nick>".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| match view.resolve_nick(&m.text) {
                Some(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "🗡️ Placed {} on {}!",
                        Mark::Assassin.styled(),
                        view.roster.seats[idx].user.nick().bold(),
                    ),
                ),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            let target = match ans {
                Answer::Ok(idx) => idx,
                Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                    rctx.roster.neighbors(seat).1
                }
            };
            board.set_mark(target, Mark::Assassin);
            let mut state = board.get_key::<AssassinDuskState>();
            state.apprentice_placed_mark = true;
            board.set_key(state);
            rctx.record(irc_format!(
                "🗡️ {label} ({}) placed {} on {}.",
                rctx.nick(seat).bold(),
                Mark::Assassin.styled(),
                rctx.nick(target).bold(),
            ));
        }))
    }
}

#[cfg(test)]
#[path = "assassin_test.rs"]
mod test;

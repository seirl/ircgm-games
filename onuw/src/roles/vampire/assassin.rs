use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::marks::Mark;
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect,
    SpecialVictory, Team, VictoryCtx, WakeStep,
};

pub struct Assassin;
pub static ASSASSIN: Assassin = Assassin;

pub struct ApprenticeAssassin;
pub static APPRENTICE_ASSASSIN: ApprenticeAssassin = ApprenticeAssassin;

#[derive(Clone, Copy, Debug)]
struct ApprenticeSoloMark(bool);

impl BoardKey for ApprenticeSoloMark {
    const ACCESS: u16 = BoardAccess::CUSTOM_1;

    fn init() -> Self {
        Self(false)
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
            &[WakeStep::solo(&ASSASSIN, 44, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .dusk()
                .copy_on_turn()
                .writes(BoardAccess::MARKS)]
        }
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
        let players_str = ctx.format_visible_players();
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
                WakeStep::group(&APPRENTICE_ASSASSIN, 46, |seats, ctx| {
                    Box::pin(Self::wake_group(seats, ctx))
                })
                .dusk()
                .writes(BoardAccess::MARKS | ApprenticeSoloMark::ACCESS),
            ]
        }
    }

    fn special_victory(&self, _seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let solo_marked = ctx.board.get_key::<ApprenticeSoloMark>().0;
        let won = if solo_marked {
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
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let assassins = ctx.seats_with_role(&ASSASSIN);
        if !assassins.is_empty() {
            let list = ctx.roster.format_seats(assassins);
            let app_list = ctx.roster.format_seats(active_seats);
            for s in active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "🗡️ Apprentice Assassin: Your mentor Assassin is {} — get them killed to take over the guild and win!",
                        list.as_str().bold().orange(),
                    ),
                );
            }
            for a_seat in ctx.waking_seats_with_role(&ASSASSIN) {
                ctx.dm(
                    a_seat,
                    irc_format!(
                        "🗡️ Assassin: Your Apprentice Assassin is {} — watch your back!",
                        app_list.as_str().bold().orange(),
                    ),
                );
            }
            return Ok(GroupEffect::new(move |_board, rctx| {
                let names = rctx.roster.format_seats(active_seats);
                rctx.record(irc_format!(
                    "🗡️ Apprentice Assassin ({}) and Assassin ({}) recognized each other.",
                    names.bold(),
                    list.as_str().bold(),
                ));
            }));
        }

        let actor = active_seats.first().expect("active_seats non-empty");
        let others = active_seats.without(actor);
        for s in others {
            ctx.dm(
                s,
                irc_format!(
                    "🗡️ Apprentice Assassin: No Assassin is in play! Fellow Apprentice {} is placing the {}!",
                    ctx.nick(actor).bold(),
                    Mark::Assassin.styled(),
                ),
            );
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
            board.set_mark(target, Mark::Assassin);
            board.set_key(ApprenticeSoloMark(true));
            for s in others {
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

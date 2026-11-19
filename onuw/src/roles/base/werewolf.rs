use ircgm::prelude::*;

use crate::board::{BoardMut, CenterSlot, Roster, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep,
};

pub struct Werewolf;
pub static WEREWOLF: Werewolf = Werewolf;

pub(crate) const WOLF_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Werewolf,
    |seats, ctx| Box::pin(Werewolf::wake_group(seats, ctx)),
);

enum WolfIntent {
    Pack { dream_wolves: SeatSet },
    LonePeek {
        seat: usize,
        choice: Answer<Option<CenterSlot>>,
    },
}

impl Role for Werewolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "werewolf",
            "Werewolf",
            GameSet::Base,
            Team::Werewolf,
            "See your fellow Werewolves; if you are the Lone Wolf, you may peek at 1 center card.",
        )
        .aliases(&["ww", "wolf"])
        .night_faction(NightFaction::Werewolf)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WOLF_PACK_STEP] }
    }
}

impl Werewolf {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        // Rule (Daybreak — Dream Wolf): "sticking out her thumb so the other
        // Werewolves know who she is" suppresses Lone Wolf ("If there is only
        // one Werewolf, the Werewolf may view one center card.").
        // Rule (ONUV — Mark of Fear): "If the Dream Wolf has the Mark of
        // Fear, she does not stick out her thumb during the Werewolves' turn,
        // though she still sticks out her thumb for the Minion and Squire."
        let dream_wolves = ctx.waking_seats_with_faction(NightFaction::Werewolf) - active_seats;

        let intent = if let Some(solo) = active_seats.only() {
            if !dream_wolves.is_empty() {
                ctx.dm(
                    solo,
                    irc_format!(
                        "🐺 Fellow Werewolf pack: 💤 Dream Wolf {} (asleep — you are not the Lone Wolf!).",
                        ctx.roster.format_seats(dream_wolves).bold().red(),
                    ),
                );
                WolfIntent::Pack { dream_wolves }
            } else {
                let roster = ctx.roster;
                let hint = roster.center_or_pass_hint();
                let choice = ctx
                    .ask(
                        solo,
                        irc_format!(
                            "🐺 You are the {}! Pick ONE center card to peek ({}):",
                            "LONE WOLF".bold().red(),
                            hint.as_str().bold(),
                        ),
                        move |m| {
                            if Roster::is_pass(&m.text) {
                                return AskMatch::accept_say(
                                    None,
                                    "🐺 Passed on peeking the center.",
                                );
                            }
                            match roster.parse_center_slot(&m.text) {
                                Some(slot) => AskMatch::accept_say(
                                    Some(slot),
                                    format!(
                                        "🐺 Locked in {} — card reveals when the phase ends!",
                                        slot.label(roster)
                                    ),
                                ),
                                None => AskMatch::reject(format!("Reply {hint}.").yellow()),
                            }
                        },
                    )
                    .await?;
                WolfIntent::LonePeek { seat: solo, choice }
            }
        } else {
            for w_seat in active_seats {
                let partners = active_seats.without(w_seat);
                if dream_wolves.is_empty() {
                    ctx.dm(
                        w_seat,
                        irc_format!(
                            "🐺 Fellow Werewolf pack: {}",
                            ctx.roster.format_seats(partners).bold().red()
                        ),
                    );
                } else {
                    ctx.dm(
                        w_seat,
                        irc_format!(
                            "🐺 Fellow Werewolf pack: {} · 💤 Dream Wolf (asleep): {}",
                            ctx.roster.format_seats(partners).bold().red(),
                            ctx.roster.format_seats(dream_wolves).bold().red(),
                        ),
                    );
                }
            }
            WolfIntent::Pack { dream_wolves }
        };

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(active_seats, intent, board, rctx);
        }))
    }

    fn resolve(
        active_seats: SeatSet,
        intent: WolfIntent,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        match intent {
            WolfIntent::Pack { dream_wolves } => {
                let names = ctx.roster.format_seats(active_seats);
                if dream_wolves.is_empty() {
                    ctx.record(irc_format!(
                        "🐺 Werewolves ({}) recognized each other.",
                        names.bold().red(),
                    ));
                } else {
                    let dw_names = ctx.roster.format_seats(dream_wolves);
                    ctx.record(irc_format!(
                        "🐺 Werewolves ({}) saw sleeping Dream Wolf ({}).",
                        names.bold().red(),
                        dw_names.bold().red(),
                    ));
                }
            }
            WolfIntent::LonePeek {
                seat,
                choice: Answer::Ok(Some(slot)),
            } => {
                let seen = board.peek_center(seat, slot);
                let label = slot.label(ctx.roster);
                ctx.dm(
                    seat,
                    irc_format!("🐺 {label} is: {}", seen.display_for_peek()),
                );
                ctx.record(irc_format!(
                    "🐺 Lone Wolf {} peeked {label} ({}).",
                    ctx.nick(seat).bold().red(),
                    seen.display_for_peek(),
                ));
            }
            WolfIntent::LonePeek { seat, .. } => {
                ctx.record(irc_format!(
                    "🐺 Lone Wolf {} passed on peeking the center.",
                    ctx.nick(seat).bold().red(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "werewolf_test.rs"]
mod test;

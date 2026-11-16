//! Vampire Expansion **Vampire** — Multi-Player Coven `Huddle` Coordination!
//!
//! Uses [`Bot::huddle_available`] (`[Coven]` relay) so all waking Vampires (`Vampire`,
//! `The Master`, `Count`, Copycat/Doppelgänger-Vampire) whisper privately in DM until any coven
//! member locks in `!mark <nick>` (or bare `<nick>` when solo).

use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut};
use crate::marks::Mark;
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep,
};

pub struct Vampire;
pub static VAMPIRE: Vampire = Vampire;

pub(crate) const VAMPIRE_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Vampire,
    25,
    |seats, ctx| Box::pin(Vampire::huddle(seats, ctx)),
)
.dusk()
.writes(BoardAccess::MARKS);

impl Role for Vampire {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "vampire",
            "Vampire",
            GameSet::Vampire,
            Team::Vampire,
            "Wake in Dusk with the Vampire coven and bestow the Mark of the Vampire on a non-Vampire player!",
        )
        .aliases(&["vamp"])
        .night_faction(NightFaction::Vampire)
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[VAMPIRE_PACK_STEP] }
    }
}

impl Vampire {
    async fn huddle(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let full_coven = ctx.seats_with_faction(NightFaction::Vampire);
        let non_vampires: Vec<usize> = (0..ctx.roster.len())
            .filter(|s| !full_coven.contains(s))
            .collect();

        if non_vampires.is_empty() {
            return Ok(GroupEffect::noop());
        }

        let coven_cfg = AskConfig::new()
            .reply_deadline(ctx.deadline)
            .no_timeout_message();
        let mut coven = ctx.huddle("Coven".red().bold(), &active_seats).await?;

        let chosen = if coven.is_empty() {
            non_vampires[fastrand::usize(..non_vampires.len())]
        } else {
            let solo_coven = coven.len() == 1;
            let coven_names = ctx.roster.format_indices(&full_coven);
            let non_vamps_str = ctx.roster.format_indices(&non_vampires);
            let prompt = irc_format!(
                "{} {} — Whisper here to coordinate, or type {} to turn a non-Vampire! Non-Vampires: {non_vamps_str}",
                "🧛 Vampire Coven:".bold().red(),
                coven_names.as_str().bold(),
                "!mark <nick>".bold(),
            );

            let valid_targets = non_vampires.clone();
            let roster = ctx.roster;
            let ans = coven
                .ask(prompt, move |m| {
                    let arg = if let Some(a) = m.cmd_arg("!mark") {
                        a
                    } else if m.is_cmd("!mark") {
                        return AskMatch::reject("Usage: !mark <nick>.".yellow());
                    } else if solo_coven {
                        m.text.trim()
                    } else {
                        return AskMatch::ignore();
                    };

                    match roster.resolve_nick(arg) {
                        Some(idx) if valid_targets.contains(&idx) => AskMatch::accept_say(
                            idx,
                            irc_format!(
                                "🧛 {} bestowed the {} upon {}!",
                                m.sender.nick().bold(),
                                Mark::Vampire.styled(),
                                roster.seats[idx].user.nick().bold(),
                            ),
                        ),
                        Some(_) => AskMatch::reject("Pick a NON-Vampire player!".yellow()),
                        None => AskMatch::reject("Unknown player nickname.".yellow()),
                    }
                })
                .config(&coven_cfg)
                .await?;

            match ans {
                Answer::Ok(idx) => idx,
                Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                    let auto = non_vampires[fastrand::usize(..non_vampires.len())];
                    let _ = coven.say(format!(
                        "⏳ Time expired — randomly marked {}!",
                        ctx.nick(auto)
                    ));
                    auto
                }
            }
        };

        if !coven.is_empty() {
            tokio::time::sleep_until(ctx.deadline).await;
        }
        drop(coven);
        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(&active_seats, chosen, board, rctx);
        }))
    }

    fn resolve(
        active_seats: &[usize],
        target: usize,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        board.set_mark(target, Mark::Vampire);
        let actors = ctx.roster.format_indices(active_seats);
        ctx.record(irc_format!(
            "🧛 Vampire Coven ({}) bestowed {} on {}.",
            actors.bold().red(),
            Mark::Vampire.styled(),
            ctx.nick(target).bold(),
        ));
    }
}

#[cfg(test)]
#[path = "vampire_test.rs"]
mod test;

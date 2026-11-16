//! Base ONUW **Doppelgänger** — polymorphic over any other [`Role`].

use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Doppelganger;
pub static DOPPELGANGER: Doppelganger = Doppelganger;

impl Role for Doppelganger {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "doppelganger",
            "Doppelgänger",
            GameSet::Base,
            Team::Village,
            "Wake 1st: look at another player's card and permanently become that role & team.",
        )
        .aliases(&["dg", "doppel", "doppelgänger"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::doppelganger(&DOPPELGANGER, |s, ctx| Box::pin(Self::prompt(s, ctx)))] }
    }
}

impl Doppelganger {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let candidates = ctx.roster.all_seats().without(seat) - ctx.shielded_seats();
        if candidates.is_empty() {
            return Ok(SeatEffect::noop());
        }

        let players_str = ctx.format_visible_players();
        let view = ctx.view();
        let ans = ctx
            .ask(
                irc_format!(
                    "🎭 Doppelgänger: Pick ONE other player ({}) to copy! Players: {players_str}",
                    "<nick>".bold(),
                ),
                move |m| match view.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => {
                        AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                    }
                    Some(idx) if view.is_shielded(idx) => AskMatch::reject(
                        "🛡️ That player's card is shielded by the Sentinel!".yellow(),
                    ),
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🎭 Locked in copying {} — role reveals when the phase ends!",
                            view.roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Unknown player nickname.".yellow()),
                },
            )
            .await?;

        Ok(SeatEffect::new(move |_label, board, rctx| {
            Self::resolve(seat, ans.ok(), board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        chosen: Option<usize>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let candidates = board.unshielded_other_seats(seat);
        if candidates.is_empty() {
            return;
        }

        let (target_idx, was_random) = match chosen.filter(|&idx| candidates.contains(idx)) {
            Some(idx) => (idx, false),
            None => (
                candidates.choose_random().expect("candidates non-empty"),
                true,
            ),
        };

        let Some(copied) = board.copy_from_player(seat, target_idx, "Doppelgänger", ctx) else {
            return;
        };

        let actor_nick = ctx.nick(seat);
        let target_nick = ctx.nick(target_idx);
        let self_styled = DOPPELGANGER.info().styled_name();

        if was_random {
            ctx.dm(seat, format!("🎲 Randomly looked at {target_nick}."));
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) randomly copied {} ({}).",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                copied.styled_name(),
            ));
        } else {
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) copied {} and became {}.",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                copied.styled_name(),
            ));
        }

        ctx.dm(
            seat,
            irc_format!(
                "🎭 {}'s card is {}! You are now that role.",
                target_nick.bold(),
                copied.styled_name(),
            ),
        );
        if copied.doppelganger_acts_immediately() {
            // Immediate prompt arrives in the next tier.
        } else if copied.doppelganger_wakes_on_schedule() {
            ctx.dm(
                seat,
                "🌙 Stand by—your copied role's night info will arrive on schedule!",
            );
        } else {
            ctx.dm(seat, "🌙 You have no further night actions.");
        }
    }
}

#[cfg(test)]
#[path = "doppelganger_test.rs"]
mod test;

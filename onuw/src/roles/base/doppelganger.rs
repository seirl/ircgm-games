//! Base ONUW **Doppelgänger** — polymorphic over any other [`Role`].

use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::role::{
    GameSet, Phase, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

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
        const { &[WakeStep::player_copier(&DOPPELGANGER, |s, ctx| Box::pin(Self::prompt(s, ctx)))] }
    }
}

impl Doppelganger {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let candidates = ctx.roster.all_seats().without(seat) - ctx.shielded_seats();
        if candidates.is_empty() {
            ctx.say("🛡️ Doppelgänger: Every other player's card is shielded — you cannot copy anyone tonight!".yellow());
            return Ok(SeatEffect::new(move |_label, _board, rctx| {
                rctx.record(irc_format!(
                    "🎭 {} ({}) could not copy anyone because all other cards were shielded.",
                    DOPPELGANGER.info().styled_name(),
                    rctx.nick(seat).bold(),
                ));
            }));
        }

        let players_str = ctx.roster.format_players();
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

        let Some(copied_card) = board.copy_from_player(seat, target_idx, ctx) else {
            return;
        };

        let seen_face = copied_card.visible_role();
        let endgame_role = copied_card.effective_role();
        let actor_nick = ctx.nick(seat);
        let target_nick = ctx.nick(target_idx);
        let self_styled = DOPPELGANGER.info().styled_name();
        let roster_has_dusk = ctx
            .roster
            .deck
            .iter()
            .any(|r| r.steps().iter().any(|st| st.phase == Phase::Dusk));

        if seen_face.is_player_copier() {
            if was_random {
                ctx.dm(seat, format!("🎲 Randomly looked at {target_nick}."));
            }
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) looked at {} ({}) and stayed a sleeping Village Doppelgänger.",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                seen_face.styled_name(),
            ));
            ctx.dm(
                seat,
                irc_format!(
                    "🎭 {}'s card is {}! You remain on the Village team with no further night actions.",
                    target_nick.bold(),
                    seen_face.styled_name(),
                ),
            );
            return;
        }

        if copied_card.is_secret_copy() {
            let detail = if endgame_role.id() == seen_face.id() {
                seen_face.styled_name()
            } else {
                irc_format!("{} → {}", seen_face.styled_name(), endgame_role.styled_name())
            };
            if was_random {
                ctx.dm(seat, format!("🎲 Randomly looked at {target_nick}."));
                ctx.record(irc_format!(
                    "🎭 {self_styled} ({}) randomly copied {} ({detail}).",
                    actor_nick.bold(),
                    target_nick.as_str().bold(),
                ));
            } else {
                ctx.record(irc_format!(
                    "🎭 {self_styled} ({}) copied {} ({detail}).",
                    actor_nick.bold(),
                    target_nick.as_str().bold(),
                ));
            }
            ctx.dm(
                seat,
                irc_format!(
                    "🎭 {}'s card is {}! You silently adopt whatever center role Copycat viewed, with no further night actions.",
                    target_nick.bold(),
                    seen_face.styled_name(),
                ),
            );
            return;
        }

        let role = endgame_role;
        if was_random {
            ctx.dm(seat, format!("🎲 Randomly looked at {target_nick}."));
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) randomly copied {} ({}).",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                role.styled_name(),
            ));
        } else {
            ctx.record(irc_format!(
                "🎭 {self_styled} ({}) copied {} and became {}.",
                actor_nick.bold(),
                target_nick.as_str().bold(),
                role.styled_name(),
            ));
        }

        ctx.dm(
            seat,
            irc_format!(
                "🎭 {}'s card is {}! You are now that role.",
                target_nick.bold(),
                role.styled_name(),
            ),
        );
        if let Some(imm) = role.immediate_copy_step() {
            if roster_has_dusk && imm.order >= 100 {
                ctx.dm(
                    seat,
                    "🌙 Stand by—your copied Night action will fire at the start of Night!",
                );
            }
        } else if role.wakes_after_player_copy(roster_has_dusk) {
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

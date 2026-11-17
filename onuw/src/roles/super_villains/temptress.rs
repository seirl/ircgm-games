use ircgm::prelude::*;

use super::henchman::{HENCHMAN, VILLAIN_PACK_STEP};
use crate::board::{BoardAccess, BoardKey, BoardMut, Card, SeatSet};
use crate::role::{
    GameSet, NightFaction, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep,
};

#[derive(Clone, Copy, Debug)]
struct ExtraVillainCard(Card);

impl BoardKey for ExtraVillainCard {
    const ACCESS: u16 = BoardAccess::CUSTOM_1;

    fn init() -> Self {
        Self(Card::new(&HENCHMAN))
    }
}

impl AsMut<Card> for ExtraVillainCard {
    fn as_mut(&mut self) -> &mut Card {
        &mut self.0
    }
}

pub struct Temptress;
pub static TEMPTRESS: Temptress = Temptress;

impl Role for Temptress {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "temptress",
            "Temptress",
            GameSet::SuperVillains,
            Team::SuperVillain,
            "Wake with the Super Villains, then MUST swap the extra Villain card onto a non-Villain player!",
        )
        .night_faction(NightFaction::SuperVillain)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                VILLAIN_PACK_STEP,
                WakeStep::solo(&TEMPTRESS, 128, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .copy_on_turn()
                    .writes(BoardAccess::PLAYER_FACES | ExtraVillainCard::ACCESS),
            ]
        }
    }
}

impl Temptress {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let view = ctx.view();
        let villains = ctx.waking_seats_with_faction(NightFaction::SuperVillain);
        let non_villains = roster.all_seats().without(seat) - villains;
        let valid_targets = non_villains - view.shielded_seats();

        if valid_targets.is_empty() {
            ctx.say("💋 Temptress: No valid non-Villain targets available!");
            return Ok(SeatEffect::noop());
        }

        let non_villains_str = ctx.roster.format_seats(non_villains);
        let prompt = irc_format!(
            "💋 Temptress: You MUST plant the extra Villain card on a non-Villain player ({})! Non-Villains: {non_villains_str}",
            "<nick>".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                Some(idx) if valid_targets.contains(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "💋 Lured {} into Villainy!",
                        roster.seats[idx].user.nick().bold().red(),
                    ),
                ),
                Some(idx) if view.is_shielded(idx) => {
                    AskMatch::reject("🛡️ That player is shielded!".yellow())
                }
                Some(_) => AskMatch::reject("Pick a non-Villain player!".yellow()),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            })
            .await?;

        let (chosen, timed_out) = match ans.ok() {
            Some(idx) => (idx, false),
            None => (
                valid_targets.choose_random().expect("valid_targets non-empty"),
                true,
            ),
        };

        let fellow_villains = villains.without(seat);
        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, chosen, timed_out, fellow_villains, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        target: usize,
        timed_out: bool,
        fellow_villains: SeatSet,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Some((old_card, planted)) =
            board.swap_player_with_extra::<ExtraVillainCard>(seat, target)
        {
            if timed_out {
                ctx.dm(
                    seat,
                    irc_format!(
                        "💋 Time's up — automatically planted the extra Villain card onto {}!",
                        ctx.nick(target).bold(),
                    ),
                );
            }
            for v in fellow_villains {
                ctx.dm(
                    v,
                    irc_format!(
                        "💋 Syndicate Update: {} ({}) planted the extra Villain card onto {}!",
                        actor_label.bold().red(),
                        ctx.nick(seat).bold(),
                        ctx.nick(target).bold(),
                    ),
                );
            }
            ctx.record(irc_format!(
                "💋 {actor_label} ({}) planted {} onto {} (replacing {}).",
                ctx.nick(seat).bold(),
                planted.display_for_peek(),
                ctx.nick(target).bold(),
                old_card.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "temptress_test.rs"]
mod test;

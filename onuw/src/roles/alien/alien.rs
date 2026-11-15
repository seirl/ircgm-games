use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, ResolveCtx, Role, RoleInfo, RoleTag,
    SpecialVictory, Team, VictoryCtx, Wake,
};

pub struct Alien;
pub static ALIEN: Alien = Alien;

enum AlienDirective {
    None,
    Scan { actor: usize, target: usize },
    TractorBeam { actor: usize, a: usize, b: usize },
}

impl Role for Alien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "alien",
            "Alien",
            GameSet::Alien,
            Team::Alien,
            "Wake with the Aliens; the commander may scan <nick> a non-Alien's card OR beam <nick1> <nick2> to swap two non-Aliens!",
        )
        .tags(&[RoleTag::AlienPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(110, Self::wake_group).mutates())
    }
}

impl Alien {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let visible_aliens = ctx.seats_with_tag(RoleTag::AlienPack);
            let alien_list = ctx.roster.format_indices(&visible_aliens);

            for &s in &active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "👽 Mother Ship Scan — Aliens aboard: {}",
                        alien_list.as_str().bold().green(),
                    ),
                );
            }

            let non_aliens: Vec<usize> = (0..ctx.roster.len())
                .filter(|s| !visible_aliens.contains(s) && !ctx.shielded.contains(s))
                .collect();
            let directive = if non_aliens.is_empty() {
                AlienDirective::None
            } else {
                let commander = active_seats[0];
                let (roster, shielded) = (ctx.roster, ctx.shielded);
                let prompt = irc_format!(
                    "👽 Mother Ship Directive: Reply {} (or {}) to bio-scan a non-Alien, {} to swap two non-Aliens, or {}! Players: {}",
                    "scan <nick>".bold(),
                    "<nick>".bold(),
                    "beam <nick1> <nick2>".bold(),
                    "pass".bold(),
                    roster.format_players_marking_shields(shielded),
                );
                let ans = ctx
                    .ask(commander, prompt, move |m| {
                        if Roster::is_pass(&m.text) {
                            return AskMatch::accept(AlienDirective::None);
                        }
                        let parts: Vec<&str> = m.text.split_whitespace().collect();
                        if parts.first().is_some_and(|w| {
                            w.eq_ignore_ascii_case("beam") || w.eq_ignore_ascii_case("swap")
                        }) {
                            if parts.len() != 3 {
                                return AskMatch::reject("Usage: beam <nick1> <nick2>.".yellow());
                            }
                            let (Some(a), Some(b)) = (
                                roster.resolve_nick(parts[1]),
                                roster.resolve_nick(parts[2]),
                            ) else {
                                return AskMatch::reject("Unknown player nickname.".yellow());
                            };
                            if a == b {
                                return AskMatch::reject("Pick two DIFFERENT players!".yellow());
                            }
                            if shielded.contains(&a) || shielded.contains(&b) {
                                return AskMatch::reject(
                                    "🛡️ Shielded players cannot be tractor-beamed!".yellow(),
                                );
                            }
                            if !non_aliens.contains(&a) || !non_aliens.contains(&b) {
                                return AskMatch::reject("Pick two non-Alien players!".yellow());
                            }
                            return AskMatch::accept_say(
                                AlienDirective::TractorBeam {
                                    actor: commander,
                                    a,
                                    b,
                                },
                                irc_format!(
                                    "🛸 Tractor Beam locked in: swapping {} and {}!",
                                    roster.seats[a].user.nick().bold(),
                                    roster.seats[b].user.nick().bold(),
                                ),
                            );
                        }

                        let tok = if parts.first().is_some_and(|w| w.eq_ignore_ascii_case("scan")) {
                            if parts.len() != 2 {
                                return AskMatch::reject("Usage: scan <nick>.".yellow());
                            }
                            parts[1]
                        } else if parts.len() == 1 {
                            parts[0]
                        } else {
                            return AskMatch::reject(
                                "Reply scan <nick>, beam <nick1> <nick2>, or pass.".yellow(),
                            );
                        };

                        match roster.resolve_nick(tok) {
                            Some(idx) if non_aliens.contains(&idx) => {
                                AskMatch::accept(AlienDirective::Scan {
                                    actor: commander,
                                    target: idx,
                                })
                            }
                            Some(idx) if shielded.contains(&idx) => {
                                AskMatch::reject("🛡️ That player's card is shielded!".yellow())
                            }
                            Some(_) => AskMatch::reject("Pick a non-Alien player!".yellow()),
                            None => AskMatch::reject("Unknown player nickname.".yellow()),
                        }
                    })
                    .await?;

                ans.ok().unwrap_or(AlienDirective::None)
            };

            Ok(GroupEffect::new(move |board, rctx| {
                Self::resolve(&active_seats, directive, board, rctx);
            }))
        })
    }

    fn resolve(
        active_seats: &[usize],
        action: AlienDirective,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        match action {
            AlienDirective::Scan { actor, target } if !board.is_shielded(target) => {
                board.record_card_interaction(actor);
                let seen = board.players[target];
                let target_nick = ctx.nick(target);
                ctx.dm(
                    actor,
                    irc_format!(
                        "👽 Bio-Scan Result: {} holds {}!",
                        target_nick.as_str().bold(),
                        seen.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "👽 {} ({}) bio-scanned {} and saw {}.",
                    "Alien".bold().red(),
                    ctx.nick(actor).bold(),
                    target_nick.bold(),
                    seen.display_for_peek(),
                ));
            }
            AlienDirective::TractorBeam { actor, a, b }
                if !board.is_shielded(a) && !board.is_shielded(b) =>
            {
                board.record_card_interaction(actor);
                let card_a = board.players[a];
                let card_b = board.players[b];
                board.players.swap(a, b);
                ctx.record(irc_format!(
                    "🛸 {} ({}) tractor-beamed a swap between {} ({}) and {} ({}).",
                    "Alien".bold().red(),
                    ctx.nick(actor).bold(),
                    ctx.nick(a).bold(),
                    card_a.display_for_peek(),
                    ctx.nick(b).bold(),
                    card_b.display_for_peek(),
                ));
            }
            _ => {
                let pack_names = ctx.roster.format_indices(active_seats);
                ctx.record(irc_format!(
                    "👽 Aliens ({}) woke aboard the Mother Ship.",
                    pack_names.bold(),
                ));
            }
        }
    }
}

pub struct SyntheticAlien;
pub static SYNTHETIC_ALIEN: SyntheticAlien = SyntheticAlien;

impl Role for SyntheticAlien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "synthetic_alien",
            "Synthetic Alien",
            GameSet::Alien,
            Team::Independent,
            "You appear as an Alien to other Aliens, Leader, & Cow, but you ONLY win if YOU are killed!",
        )
        .aliases(&["synth", "synthalien"])
        .tags(&[RoleTag::AlienPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(111, Self::wake_group))
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        ctx.is_dead(seat)
            .then_some(SpecialVictory::independent_blocking(Team::MONSTER_TEAMS))
    }
}

impl SyntheticAlien {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let visible_aliens = ctx.seats_with_tag(RoleTag::AlienPack);
            let list = ctx.roster.format_indices(&visible_aliens);
            for &s in &active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "🤖👽 Synthetic Alien: You blended into the Alien scan ({})! Get YOURSELF killed at Dawn to win!",
                        list.as_str().bold().orange(),
                    ),
                );
            }
            Ok(GroupEffect::new(move |_board, rctx| {
                let names = rctx.roster.format_indices(&active_seats);
                rctx.record(irc_format!(
                    "🤖 {} ({}) infiltrated the Alien scan.",
                    SYNTHETIC_ALIEN.info().styled_name(),
                    names.bold(),
                ));
            }))
        })
    }
}

#[cfg(test)]
#[path = "alien_test.rs"]
mod test;

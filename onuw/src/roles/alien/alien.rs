use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, SpecialVictory,
    Team, VictoryCtx, WakeStep,
};

pub struct Alien;
pub static ALIEN: Alien = Alien;

pub(crate) const ALIEN_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Alien,
    110,
    |seats, ctx| Box::pin(Alien::wake_group(seats, ctx)),
)
.writes(BoardAccess::PLAYER_FACES);

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
        .night_faction(NightFaction::Alien)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP] }
    }
}

impl Alien {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let visible_aliens = ctx.waking_seats_with_faction(NightFaction::Alien);
        let alien_list = ctx.roster.format_seats(visible_aliens);
        let cow_seats = ctx.waking_seats_with_role(&super::cow::COW);
        let cow_list = ctx.roster.format_seats(cow_seats);

        for s in active_seats {
            ctx.dm(
                s,
                irc_format!(
                    "👽 Mother Ship Scan — Aliens aboard: {}",
                    alien_list.as_str().bold().green(),
                ),
            );
            if !cow_seats.is_empty() {
                ctx.dm(
                    s,
                    irc_format!(
                        "🐄🛸 Alien Hoof-Scan: {} holds the Cow card!",
                        cow_list.as_str().bold().yellow(),
                    ),
                );
            }
            if ctx.waking_roles[s].id() == "synthetic_alien" {
                ctx.dm(
                    s,
                    "🤖👽 Synthetic Alien: You blended into the Alien scan! Get YOURSELF killed in the vote to win!",
                );
            }
        }

        let shielded = ctx.shielded_seats();
        let non_alien_seats = ctx.roster.all_seats() - visible_aliens;
        let non_aliens = non_alien_seats - shielded;
        let directive = if non_aliens.is_empty() {
            AlienDirective::None
        } else {
            let commander = active_seats
                .into_iter()
                .find(|&s| ctx.waking_roles[s].id() == "alien")
                .or_else(|| {
                    active_seats
                        .into_iter()
                        .find(|&s| ctx.waking_roles[s].id() != "synthetic_alien")
                })
                .unwrap_or_else(|| active_seats.first().expect("active_seats non-empty"));
            let roster = ctx.roster;
            let non_aliens_str = ctx.format_visible_seats(non_alien_seats);
            let prompt = irc_format!(
                "👽 Mother Ship Directive: Reply {} (or {}) to bio-scan a non-Alien, {} to swap two non-Aliens, or {}! Non-Aliens: {non_aliens_str}",
                "scan <nick>".bold(),
                "<nick>".bold(),
                "beam <nick1> <nick2>".bold(),
                "pass".bold(),
            );
            let ans = ctx
                .ask(commander, prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept_say(
                            AlienDirective::None,
                            "👽 Passed — Mother Ship standing by.",
                        );
                    }
                    let parts: Vec<&str> = m.text.split_whitespace().collect();
                    if parts.first().is_some_and(|w| {
                        w.eq_ignore_ascii_case("beam") || w.eq_ignore_ascii_case("swap")
                    }) {
                        if parts.len() != 3 {
                            return AskMatch::reject("Usage: beam <nick1> <nick2>.".yellow());
                        }
                        let (Some(a), Some(b)) =
                            (roster.resolve_nick(parts[1]), roster.resolve_nick(parts[2]))
                        else {
                            return AskMatch::reject("Unknown player nickname.".yellow());
                        };
                        if a == b {
                            return AskMatch::reject("Pick two DIFFERENT players!".yellow());
                        }
                        if shielded.contains(a) || shielded.contains(b) {
                            return AskMatch::reject(
                                "🛡️ Shielded players cannot be tractor-beamed!".yellow(),
                            );
                        }
                        if !non_aliens.contains(a) || !non_aliens.contains(b) {
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

                    let tok = if parts
                        .first()
                        .is_some_and(|w| w.eq_ignore_ascii_case("scan"))
                    {
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
                        Some(idx) if non_aliens.contains(idx) => AskMatch::accept_say(
                            AlienDirective::Scan {
                                actor: commander,
                                target: idx,
                            },
                            irc_format!(
                                "👽 Bio-Scan locked in on {} — result arrives when the phase ends!",
                                roster.seats[idx].user.nick().bold(),
                            ),
                        ),
                        Some(idx) if shielded.contains(idx) => {
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
            Self::resolve(active_seats, directive, board, rctx);
        }))
    }

    fn resolve(
        active_seats: SeatSet,
        action: AlienDirective,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        match action {
            AlienDirective::Scan { actor, target } => {
                if let Some(seen) = board.peek_player(actor, target) {
                    board.record_card_viewers(active_seats);
                    let target_nick = ctx.nick(target);
                    for s in active_seats {
                        ctx.dm(
                            s,
                            irc_format!(
                                "👽 Bio-Scan Result: {} holds {}!",
                                target_nick.as_str().bold(),
                                seen.display_for_peek(),
                            ),
                        );
                    }
                    ctx.record(irc_format!(
                        "👽 {} ({}) bio-scanned {} and saw {}.",
                        "Alien".bold().red(),
                        ctx.nick(actor).bold(),
                        target_nick.bold(),
                        seen.display_for_peek(),
                    ));
                    return;
                }
            }
            AlienDirective::TractorBeam { actor, a, b } => {
                if let Some((card_a, card_b)) = board.swap_players(actor, a, b) {
                    for s in active_seats.without(actor) {
                        ctx.dm(
                            s,
                            irc_format!(
                                "🛸 Mother Ship Tractor Beam swapped {} and {}!",
                                ctx.nick(a).bold(),
                                ctx.nick(b).bold(),
                            ),
                        );
                    }
                    ctx.record(irc_format!(
                        "🛸 {} ({}) tractor-beamed a swap between {} ({}) and {} ({}).",
                        "Alien".bold().red(),
                        ctx.nick(actor).bold(),
                        ctx.nick(a).bold(),
                        card_a.display_for_peek(),
                        ctx.nick(b).bold(),
                        card_b.display_for_peek(),
                    ));
                    return;
                }
            }
            AlienDirective::None => {}
        }
        let pack_names = ctx.roster.format_seats(active_seats);
        ctx.record(irc_format!(
            "👽 Aliens ({}) woke aboard the Mother Ship.",
            pack_names.bold(),
        ));
    }
}

pub struct SyntheticAlien;
pub static SYNTHETIC_ALIEN: SyntheticAlien = SyntheticAlien;

const ALL_TEAMS: &[Team] = &[
    Team::Village,
    Team::Werewolf,
    Team::Vampire,
    Team::Alien,
    Team::SuperVillain,
];

impl Role for SyntheticAlien {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "synthetic_alien",
            "Synthetic Alien",
            GameSet::Alien,
            Team::Independent,
            "Wake with the Aliens and appear Alien to Leader & Cow, but you ONLY win if YOU are killed!",
        )
        .aliases(&["synth", "synthalien"])
        .night_faction(NightFaction::Alien)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[ALIEN_PACK_STEP] }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        ctx.is_dead(seat)
            .then_some(SpecialVictory::independent_blocking(ALL_TEAMS))
    }
}

#[cfg(test)]
#[path = "alien_test.rs"]
mod test;

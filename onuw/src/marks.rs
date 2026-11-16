//! Vampire expansion **Mark** tokens (`Mark::Clarity`, `Mark::Vampire`, `Mark::Bat`, etc.),
//! Dusk-end inspection / `Fear` / `Lovers` boundary logic, and [`EndgameModifier`] hooks.

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster, SeatSet};
use crate::role::{
    DeathCtx, DeathEffect, EffectiveAlignment, EndgameModifier, NightWakers, Phase, StepKind,
    Team, TeamBlock, WinnerFilterCtx, WinnerMap,
};

/// Vampire expansion Marks (every player starts Dusk with `Mark::Clarity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Clarity,
    Vampire,
    Bat,
    Fear,
    Disease,
    Love,
    Traitor,
    Assassin,
}

impl Mark {
    pub fn name(self) -> &'static str {
        match self {
            Mark::Clarity => "Mark of Clarity",
            Mark::Vampire => "Mark of the Vampire",
            Mark::Bat => "Mark of the Bat",
            Mark::Fear => "Mark of Fear",
            Mark::Disease => "Mark of Disease",
            Mark::Love => "Mark of Love",
            Mark::Traitor => "Mark of the Traitor",
            Mark::Assassin => "Mark of the Assassin",
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Mark::Vampire => self.name().bold().red().into(),
            Mark::Fear | Mark::Disease | Mark::Assassin | Mark::Traitor => {
                self.name().bold().orange().into()
            }
            Mark::Love => self.name().bold().yellow().into(),
            Mark::Bat | Mark::Clarity => self.name().bold().light_cyan().into(),
        }
    }

    /// Runs the `00` Dusk -> Night boundary: notifies players of their Dusk `Mark`, records
    /// `Mark::Fear` paralysis for Night wakers, and wakes `Mark::Love` Lovers to recognize each other.
    pub(crate) fn finalize_dusk_boundary(
        roster: &Roster,
        board: &BoardCards,
        wakers: &mut NightWakers,
        chronicle: &mut Vec<Text>,
    ) {
        let marks_in_play =
            roster.deck_has_marks() || board.marks().iter().any(|&m| m != Mark::Clarity);

        let feared: SeatSet = roster
            .all_seats()
            .into_iter()
            .filter(|&s| board.mark(s) == Mark::Fear)
            .collect();
        wakers.feared = feared;

        if marks_in_play {
            for s in roster.all_seats() {
                let mark = board.mark(s);
                if mark == Mark::Fear {
                    let _ = roster.seats[s].user.say(irc_format!(
                        "😱 Dusk ends — your Mark is {}! You received the {} during Dusk and are too terrified to wake up tonight!",
                        Mark::Fear.styled(),
                        Mark::Fear.styled(),
                    ));
                } else {
                    let _ = roster.seats[s].user.say(irc_format!(
                        "🔖 Dusk ends — you check your Mark: {}.",
                        mark.styled(),
                    ));
                }
            }
        }

        for s in feared {
            if !marks_in_play {
                let _ = roster.seats[s].user.say(irc_format!(
                    "😱 You received the {} during Dusk and are too terrified to wake up tonight!",
                    Mark::Fear.styled(),
                ));
            }
            let r = wakers.role(s);
            let wakes_in_night = r.steps().iter().any(|st| {
                st.phase == Phase::Night
                    && st.order >= 100
                    && (matches!(st.kind, StepKind::Group(_)) || !wakers.acted_immediately.contains(s))
            });
            if wakes_in_night {
                chronicle.push(irc_format!(
                    "😱 {} was paralyzed by the {} and could not wake up during Night!",
                    roster.seats[s].user.nick().bold(),
                    Mark::Fear.styled(),
                ));
            }
        }

        // `00: Lovers` — players holding `Mark::Love` wake at the end of Dusk to recognize each other:
        let lovers: SeatSet = roster
            .all_seats()
            .into_iter()
            .filter(|&s| board.mark(s) == Mark::Love)
            .collect();
        if let Some(solo_lover) = lovers.only() {
            let _ = roster.seats[solo_lover].user.say(
                "💘 Lovers wake: You woke alone — no one else holds the Mark of Love!",
            );
        } else if lovers.len() >= 2 {
            for s in lovers {
                let partners = lovers.without(s);
                let _ = roster.seats[s].user.say(irc_format!(
                    "💘 Lovers wake: Your Lover is {} — keep each other alive!",
                    roster.format_seats(partners).bold().magenta(),
                ));
            }
            chronicle.push(irc_format!(
                "💘 Lovers ({}) woke at the end of Dusk and recognized each other.",
                roster.format_seats(lovers).bold().magenta(),
            ));
        }
    }
}

impl EndgameModifier for Mark {
    fn modifier_name(&self) -> &'static str {
        self.name()
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        _board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        if *self == Mark::Vampire {
            EffectiveAlignment {
                team: Team::Vampire,
                threat_team: Some(Team::Vampire),
            }
        } else {
            cur
        }
    }

    fn on_death(&self, dead_seat: usize, ctx: &DeathCtx<'_>) -> Vec<DeathEffect> {
        if *self != Mark::Love {
            return Vec::new();
        }
        (0..ctx.roster.len())
            .filter(|&other| other != dead_seat && ctx.board.mark(other) == Mark::Love)
            .map(|other| DeathEffect::Kill {
                seat: other,
                respects_protection: false,
                announcement: irc_format!(
                    "💔 {} died of heartbreak alongside their Lover!",
                    ctx.roster.seats[other].user.nick().bold().red(),
                ),
            })
            .collect()
    }

    fn override_win(&self, holder: usize, ctx: &WinnerFilterCtx<'_>, winners: &mut WinnerMap) {
        if *self != Mark::Traitor {
            return;
        }
        let team = ctx.board.effective_team(holder);
        if team != Team::Independent {
            let teammates: SeatSet = ctx
                .roster
                .all_seats()
                .without(holder)
                .into_iter()
                .filter(|&o| ctx.board.effective_team(o) == team)
                .collect();
            if !teammates.is_empty() {
                let traitor_won = ctx.team_block != TeamBlock::AllTeams
                    && !ctx.dead_seats.contains(holder)
                    && teammates.intersects(ctx.dead_seats);
                if traitor_won {
                    winners.insert(holder, "Traitor");
                } else {
                    winners.remove(&holder);
                }
            }
        }
    }

    fn apply_forfeits(
        &self,
        holder: usize,
        ctx: &mut WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        if *self == Mark::Disease {
            for voter in 0..ctx.roster.len() {
                if ctx.votes[voter] == Some(holder) {
                    winners.remove(&voter);
                }
            }
        }
    }
}

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
    pub const ALL: &'static [Mark] = &[
        Mark::Clarity,
        Mark::Vampire,
        Mark::Bat,
        Mark::Fear,
        Mark::Disease,
        Mark::Love,
        Mark::Traitor,
        Mark::Assassin,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Mark::Clarity => "mark_of_clarity",
            Mark::Vampire => "mark_of_the_vampire",
            Mark::Bat => "mark_of_the_bat",
            Mark::Fear => "mark_of_fear",
            Mark::Disease => "mark_of_disease",
            Mark::Love => "mark_of_love",
            Mark::Traitor => "mark_of_the_traitor",
            Mark::Assassin => "mark_of_the_assassin",
        }
    }

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

    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Mark::Clarity => &["clarity", "clear", "clean", "mark of the clarity"],
            Mark::Vampire => &["vampire", "vamp", "mark of vampire"],
            Mark::Bat => &["bat", "mark of bat"],
            Mark::Fear => &["fear", "mark of the fear"],
            Mark::Disease => &["disease", "diseased", "mark of the disease"],
            Mark::Love => &["love", "lover", "lovers", "mark of the love"],
            Mark::Traitor => &["traitor", "mark of traitor"],
            Mark::Assassin => &["assassin", "mark of assassin"],
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Mark::Clarity => "Initial / Priest",
            Mark::Vampire => "Vampire Coven",
            Mark::Bat => "Renfield",
            Mark::Fear => "The Count",
            Mark::Disease => "Diseased",
            Mark::Love => "Cupid",
            Mark::Traitor => "Instigator",
            Mark::Assassin => "Assassin",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Mark::Clarity => {
                "Default starting Mark with no special effect; Priest cleanses Dusk Marks back to Clarity."
            }
            Mark::Vampire => {
                "Turns the holder into a Vampire on the Vampire team (unless overridden by a role-changing Artifact)."
            }
            Mark::Bat => {
                "Placed by Renfield on themselves in Dusk (overwriting earlier Marks); has no additional endgame effect."
            }
            Mark::Fear => {
                "Placed on a non-Vampire in Dusk; prevents the holder from waking up to act during Night."
            }
            Mark::Disease => {
                "Placed on a neighbor in Dusk; anyone who votes for the holder cannot win (even if their team wins)."
            }
            Mark::Love => {
                "Lovers wake at the end of Dusk to recognize each other; if one Lover dies, the other dies of heartbreak too."
            }
            Mark::Traitor => {
                "If at least one other player shares your effective team, you win ONLY if a teammate dies."
            }
            Mark::Assassin => {
                "Marks a target in Dusk; Assassin (or solo Apprentice Assassin) wins ONLY if the holder dies."
            }
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
                    && crate::roles::is_night_step(st, true)
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
        if *self == Mark::Traitor {
            apply_traitor_win_override(holder, ctx, winners);
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
                if ctx.votes[voter] == holder {
                    winners.remove(&voter);
                }
            }
        }
    }
}

/// Shared Traitor win-condition evaluation for `Mark::Traitor` and `Artifact::DaggerOfTheTraitor`:
/// when at least one other player shares the holder's effective team, the holder wins iff
/// at least one teammate dies (and no global `AllTeams` block / `Oracle Slain` override applies).
pub(crate) fn apply_traitor_win_override(
    holder: usize,
    ctx: &WinnerFilterCtx<'_>,
    winners: &mut WinnerMap,
) {
    if ctx.hunted_active {
        return;
    }
    let team = ctx.board.effective_team(holder);
    if team == Team::Independent {
        return;
    }
    let teammates = ctx.board.seats_on_team(team).without(holder);
    if !teammates.is_empty() {
        let blocked_by_other = ctx.team_block == TeamBlock::AllTeams && team != Team::Mad;
        if !blocked_by_other && teammates.intersects(ctx.dead_seats) {
            winners.insert(holder, "Traitor");
        } else {
            winners.remove(&holder);
        }
    }
}

fn normalize_mark_key(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Resolves a [`Mark`] by ID, display name, or alias (case- and punctuation-insensitive).
pub fn find_mark(query: &str) -> Option<Mark> {
    let key = normalize_mark_key(query);
    if key.is_empty() {
        return None;
    }
    for &mark in Mark::ALL {
        if normalize_mark_key(mark.id()) == key || normalize_mark_key(mark.name()) == key {
            return Some(mark);
        }
        for &alias in mark.aliases() {
            if normalize_mark_key(alias) == key {
                return Some(mark);
            }
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::{find_mark, Mark};

    #[test]
    fn all_mark_ids_names_and_aliases_are_unique_and_resolvable() {
        assert_eq!(Mark::ALL.len(), 8);
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for &mark in Mark::ALL {
            assert!(ids.insert(mark.id()), "duplicate mark id `{}`", mark.id());
            assert!(
                names.insert(mark.name()),
                "duplicate mark name `{}`",
                mark.name()
            );
            assert!(!mark.summary().contains('`'));
            assert!(!mark.source().contains('`'));
            assert_eq!(find_mark(mark.id()), Some(mark));
            assert_eq!(find_mark(mark.name()), Some(mark));
            for &alias in mark.aliases() {
                assert_eq!(
                    find_mark(alias),
                    Some(mark),
                    "alias `{alias}` did not resolve to `{mark:?}`"
                );
            }
        }
        assert_eq!(find_mark(""), None);
        assert_eq!(find_mark("unknown_mark"), None);
    }
}

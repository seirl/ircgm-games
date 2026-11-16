//! **Voting** resolution, chain-kills, multi-faction victory evaluation, and deferred `!reveal` chronicle.

use std::collections::{HashMap, HashSet};

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::game::GameSummary;
use crate::marks::Mark;
use crate::role::{DeathCtx, DeathEffect, SpecialVictory, Team, VictoryCtx, WinnerFilterCtx, WinnerMap};

/// Deferred full game record (center cards, Dusk/Night chronicle, and per-seat `start → end`
/// roles/marks/artifacts) held back after Voting until `!reveal` is run in channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReveal {
    lines: Vec<Text>,
}

impl GameReveal {
    pub fn lines(&self) -> &[Text] {
        &self.lines
    }

    pub fn into_lines(self) -> Vec<Text> {
        self.lines
    }

    pub fn announce(&self, room: &Room) -> Result<()> {
        for line in &self.lines {
            room.say(line.clone())?;
        }
        Ok(())
    }
}

pub fn resolve_and_announce_winners(
    room: &Room,
    roster: &Roster,
    board: BoardCards,
    chronicle: Vec<Text>,
    votes: Vec<Option<usize>>,
    day_channel_log: &[ChatMessage],
) -> Result<GameSummary> {
    let n = roster.len();
    let mut tally = vec![0usize; n];
    for &opt in &votes {
        if let Some(t) = opt {
            tally[t] += 1;
        }
    }

    let death_ctx = DeathCtx {
        roster,
        board: &board,
        votes: &votes,
    };

    // Determine active monster factions in front of players (`Epic Battle` when >= 2 monster factions clash):
    let threats_by_faction: Vec<(Team, Vec<usize>)> = Team::MONSTER_TEAMS
        .iter()
        .map(|&team| {
            let seats: Vec<usize> = (0..n)
                .filter(|&i| board.is_monster_threat_for(i, team))
                .collect();
            (team, seats)
        })
        .collect();
    let present_monster_factions: Vec<Team> = threats_by_faction
        .iter()
        .filter(|(_, seats)| !seats.is_empty())
        .map(|(team, _)| *team)
        .collect();
    let is_epic_battle = present_monster_factions.len() >= 2;

    // Collect unified vote protections across each seat's active EndgameModifiers:
    let mut vote_immune: HashSet<usize> = HashSet::new();
    let mut hunter_shielded: HashSet<usize> = HashSet::new();
    let mut death_announcements: Vec<Text> = Vec::new();
    for seat in 0..n {
        for modifier in board.seat_endgame_modifiers(seat) {
            if let Some(prot) = modifier.vote_protection(seat, &death_ctx) {
                vote_immune.insert(prot.protected_seat);
                if prot.blocks_hunter_shot {
                    hunter_shielded.insert(prot.protected_seat);
                }
                if tally[prot.protected_seat] >= 2
                    || (is_epic_battle && tally[prot.protected_seat] >= 1)
                {
                    death_announcements.push(prot.announcement);
                }
            }
        }
    }

    // Standard ONUW: highest vote count >= 2 among non-immune seats dies.
    // Epic Battle (>= 2 monster factions in play): BOTH most votes (>= 2) AND 2nd-most votes (>= 1)
    // die, unless 2+ players tied for most votes (in which case all tied for 1st die).
    let max_killable_votes = tally
        .iter()
        .enumerate()
        .filter(|(idx, _)| !vote_immune.contains(idx))
        .map(|(_, &c)| c)
        .max()
        .unwrap_or(0);

    let mut dead: HashSet<usize> = HashSet::new();
    if max_killable_votes >= 2 {
        let top_tied: Vec<usize> = tally
            .iter()
            .enumerate()
            .filter_map(|(idx, &c)| {
                (c == max_killable_votes && !vote_immune.contains(&idx)).then_some(idx)
            })
            .collect();
        dead.extend(&top_tied);

        if is_epic_battle && top_tied.len() == 1 {
            let second_votes = tally
                .iter()
                .enumerate()
                .filter(|(idx, &c)| !vote_immune.contains(idx) && c < max_killable_votes)
                .map(|(_, &c)| c)
                .max()
                .unwrap_or(0);
            if second_votes >= 1 {
                for (idx, &c) in tally.iter().enumerate() {
                    if c == second_votes && !vote_immune.contains(&idx) {
                        dead.insert(idx);
                    }
                }
            }
        }
    }

    // Fixpoint loop over `EndgameModifier::on_death` (`Hunter`, `Mark::Love`, etc.):
    let mut processed_deaths: HashSet<usize> = HashSet::new();
    loop {
        let mut current_dead: Vec<usize> = dead.iter().copied().collect();
        current_dead.sort_unstable();
        let mut newly_killed = Vec::new();
        for d in current_dead {
            if processed_deaths.insert(d) {
                for modifier in board.seat_endgame_modifiers(d) {
                    for effect in modifier.on_death(d, &death_ctx) {
                        let DeathEffect::Kill {
                            seat: kill_seat,
                            respects_protection,
                            announcement,
                        } = effect;
                        let blocked = respects_protection && hunter_shielded.contains(&kill_seat);
                        if !dead.contains(&kill_seat) && !newly_killed.contains(&kill_seat) && !blocked
                        {
                            newly_killed.push(kill_seat);
                            death_announcements.push(announcement);
                        }
                    }
                }
            }
        }
        if newly_killed.is_empty() {
            break;
        }
        dead.extend(newly_killed);
    }

    // Build the deferred `!reveal` record (center cards, compact per-seat roles, and Dusk & Night chronicle):
    let sep = " · ".grey();
    let mut reveal_lines = Vec::new();
    reveal_lines.push(irc_format!(
        "{} {} {}{}{} {}{}{} {}",
        "🗳️ VOTING REVEAL — Center:".bold(),
        "#1".grey(),
        board.center_card(0).display_reveal(),
        sep.clone(),
        "#2".grey(),
        board.center_card(1).display_reveal(),
        sep.clone(),
        "#3".grey(),
        board.center_card(2).display_reveal(),
    ));

    let mut role_items: Vec<Text> = Vec::with_capacity(n);
    for idx in 0..n {
        let mut token_suffix = String::new();
        let seat_mark = board.mark(idx);
        if seat_mark != Mark::Clarity {
            token_suffix.push_str(&format!(" [{}]", seat_mark.name()));
        }
        if let Some(art) = board.artifact(idx) {
            token_suffix.push_str(&format!(" {{{}}}", art.name()));
        }
        if dead.contains(&idx) {
            token_suffix.push_str(" 💀");
        }
        let nick = roster.seats[idx].user.nick();
        let card = board.player_card(idx);
        let item = if card.face_role().id() == roster.seats[idx].initial_role.id() {
            irc_format!(
                "{}: {}{token_suffix}",
                nick.as_str().bold(),
                card.display_reveal(),
            )
        } else {
            irc_format!(
                "{}: {} → {}{token_suffix}",
                nick.as_str().bold(),
                roster.seats[idx].initial_role.styled_name(),
                card.display_reveal(),
            )
        };
        role_items.push(item);
    }
    for (chunk_idx, chunk) in role_items.chunks(5).enumerate() {
        let prefix: Text = if chunk_idx == 0 {
            irc_format!("{} ", "🃏 Roles:".bold())
        } else {
            "  ".into()
        };
        reveal_lines.push(prefix + Text::join(chunk, " · ".grey()));
    }

    if chronicle.is_empty() {
        reveal_lines.push(irc_format!(
            "{} Quiet night — no cards or marks were acted upon!",
            "📜 What Happened:".bold().cyan(),
        ));
    } else {
        reveal_lines.push("📜 What Happened:".bold().cyan().into());
        for (step_idx, entry) in chronicle.iter().enumerate() {
            reveal_lines.push(irc_format!("  {}. {entry}", step_idx + 1));
        }
    }
    let reveal = GameReveal {
        lines: reveal_lines,
    };

    // 1. Announce vote tallies and who died on a single line (without spoiling roles or night actions):
    let mut voted_seats: Vec<(usize, usize)> = tally
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, c)| c > 0)
        .collect();
    voted_seats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let tally_str = if voted_seats.is_empty() {
        "no votes".to_owned()
    } else {
        voted_seats
            .iter()
            .map(|&(idx, c)| format!("{} ({c})", roster.seats[idx].user.nick()))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let outcome_span: Text = if dead.is_empty() {
        "Nobody died!".bold().green()
    } else {
        let mut sorted_dead: Vec<usize> = dead.iter().copied().collect();
        sorted_dead.sort_unstable();
        let dead_nicks = roster.format_indices(&sorted_dead);
        irc_format!("{} {}", "Killed:".bold().red(), dead_nicks.as_str().bold())
    };

    room.say(irc_format!(
        "{} Votes: {tally_str}{}{outcome_span}",
        "🗳️ VOTING —".bold(),
        sep,
    ))?;

    for msg in death_announcements {
        room.say(msg)?;
    }

    // 4. Victory Evaluation:
    let vctx = VictoryCtx {
        roster,
        board: &board,
        votes: &votes,
        dead_seats: &dead,
    };

    // 4a. Special victories across each seat's active EndgameModifiers:
    let mut forced_team_candidate: Option<(Team, Text)> = None;
    let mut blocked_teams: HashSet<Team> = HashSet::new();
    let mut winners: WinnerMap = HashMap::new();
    for seat in 0..n {
        for modifier in board.seat_endgame_modifiers(seat) {
            match modifier.victory_claim(seat, &vctx) {
                Some(SpecialVictory::ForceTeam { team, announcement })
                    if forced_team_candidate.is_none() =>
                {
                    forced_team_candidate = Some((team, announcement));
                }
                Some(SpecialVictory::Independent { blocks })
                    if board.effective_team(seat) == Team::Independent =>
                {
                    winners.insert(seat, modifier.modifier_name());
                    blocked_teams.extend(blocks.iter().copied());
                }
                _ => {}
            }
        }
    }

    let forced_team_winner = match forced_team_candidate {
        Some((team, announcement)) if !blocked_teams.contains(&team) => {
            room.say(announcement)?;
            Some(team)
        }
        _ => None,
    };

    // 4b. Multi-faction & Epic Battle victory evaluation (`Village` vs `Werewolf` / `Vampire` / `Alien` / `SuperVillain`):
    let threats_of = |team: Team| -> &[usize] {
        threats_by_faction
            .iter()
            .find(|(t, _)| *t == team)
            .map(|(_, seats)| seats.as_slice())
            .unwrap_or(&[])
    };
    let minions_for = |team: Team| -> Vec<usize> {
        (0..n)
            .filter(|&i| {
                board.effective_team(i) == team && board.player_card(i).effective_role().is_minion()
            })
            .collect()
    };

    let mut winning_teams: HashSet<Team> = HashSet::new();
    if let Some(forced) = forced_team_winner {
        winning_teams.insert(forced);
    } else {
        let village_wins = if !present_monster_factions.is_empty() {
            !blocked_teams.contains(&Team::Village)
                && present_monster_factions
                    .iter()
                    .all(|&f| threats_of(f).iter().any(|s| dead.contains(s)))
        } else {
            !blocked_teams.contains(&Team::Village) && dead.is_empty()
        };
        if village_wins {
            winning_teams.insert(Team::Village);
        }

        for &team in Team::MONSTER_TEAMS {
            if blocked_teams.contains(&team) {
                continue;
            }
            let threats = threats_of(team);
            let minions = minions_for(team);
            if !threats.is_empty() {
                let all_own_survive = threats.iter().all(|s| !dead.contains(s));
                let won = if is_epic_battle {
                    let rival_killed = present_monster_factions.iter().any(|&other| {
                        other != team && threats_of(other).iter().any(|s| dead.contains(s))
                    });
                    all_own_survive && rival_killed
                } else {
                    all_own_survive
                };
                if won {
                    winning_teams.insert(team);
                }
            } else {
                // Minion with 0 primary threats in play wins individually if they survive
                // AND another player dies (and the team is not blocked by Tanner/etc.):
                for m in minions {
                    if !dead.contains(&m) && dead.iter().any(|&d| d != m) {
                        winners.insert(m, team.win_label());
                    }
                }
            }
        }
    }

    for &team in &[
        Team::Village,
        Team::Werewolf,
        Team::Vampire,
        Team::Alien,
        Team::SuperVillain,
    ] {
        if winning_teams.contains(&team) {
            for seat in 0..n {
                if board.effective_team(seat) == team {
                    winners.insert(seat, team.win_label());
                }
            }
        }
    }

    // 4c. Win overrides (`Mark::Traitor`) followed by win-forfeit penalties (`Diseased`, `Mark::Disease`, `Artifact` conduct):
    let mut forfeit_announcements = Vec::new();
    {
        let mut fctx = WinnerFilterCtx {
            roster,
            board: &board,
            votes: &votes,
            dead_seats: &dead,
            blocked_teams: &blocked_teams,
            casemap: room.casemap(),
            day_channel_log,
            announcements: &mut forfeit_announcements,
        };
        for seat in 0..n {
            for modifier in board.seat_endgame_modifiers(seat) {
                modifier.override_win(seat, &fctx, &mut winners);
            }
        }
        for seat in 0..n {
            for modifier in board.seat_endgame_modifiers(seat) {
                modifier.apply_forfeits(seat, &mut fctx, &mut winners);
            }
        }
    }
    for msg in forfeit_announcements {
        room.say(msg)?;
    }

    let mut sorted_winners: Vec<(usize, &'static str)> = winners.into_iter().collect();
    sorted_winners.sort_by_key(|(i, _)| *i);

    if sorted_winners.is_empty() {
        room.say("💀 NOBODY WINS!".bold().red())?;
    } else {
        let list = sorted_winners
            .iter()
            .map(|&(i, label)| format!("{} ({label})", roster.seats[i].user.nick()))
            .collect::<Vec<_>>()
            .join(", ");
        room.say(irc_format!(
            "{} {}",
            "🏆 Winners:".bold().green(),
            list.as_str().bold(),
        ))?;
    }

    Ok(GameSummary {
        final_board: board,
        chronicle,
        votes,
        dead_seats: dead,
        winners: sorted_winners,
        reveal,
    })
}

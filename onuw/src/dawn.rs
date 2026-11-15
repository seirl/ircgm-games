//! **Dawn** vote resolution, chain-kills, multi-faction victory evaluation, and deferred `!reveal` chronicle.

use std::collections::{HashMap, HashSet};

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::game::GameSummary;
use crate::marks::Mark;
use crate::role::{DeathCtx, DeathEffect, SpecialVictory, Team, VictoryCtx};

/// Deferred full game record (center cards, Dusk/Night chronicle, and per-seat `start → end`
/// roles/marks/artifacts) held back after Dawn until `!reveal` is run in channel.
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

    // Collect unified vote protections (`Bodyguard`, `Prince`, `The Master`):
    let mut vote_immune: HashSet<usize> = HashSet::new();
    let mut hunter_shielded: HashSet<usize> = HashSet::new();
    let mut death_announcements: Vec<Text> = Vec::new();
    for seat in 0..n {
        if let Some(prot) = board.players[seat]
            .effective_role()
            .vote_protection(seat, &death_ctx)
        {
            vote_immune.insert(prot.protected_seat);
            if prot.blocks_hunter_shot {
                hunter_shielded.insert(prot.protected_seat);
            }
            if tally[prot.protected_seat] >= 2 {
                death_announcements.push(prot.announcement);
            }
        }
    }

    // Highest vote count >= 2 among non-immune seats dies:
    let max_killable_votes = tally
        .iter()
        .enumerate()
        .filter(|(idx, _)| !vote_immune.contains(idx))
        .map(|(_, &c)| c)
        .max()
        .unwrap_or(0);

    let mut dead: HashSet<usize> = HashSet::new();
    if max_killable_votes >= 2 {
        for (idx, &c) in tally.iter().enumerate() {
            if c == max_killable_votes && !vote_immune.contains(&idx) {
                dead.insert(idx);
            }
        }
    }

    // Fixpoint loop over `Role::on_death` (`Hunter` kill + `Diseased` taint) and `Mark::Love` heartbreak:
    let mut processed_deaths: HashSet<usize> = HashSet::new();
    let mut death_forfeits: HashSet<usize> = HashSet::new();
    loop {
        let mut current_dead: Vec<usize> = dead.iter().copied().collect();
        current_dead.sort_unstable();
        let mut newly_killed = Vec::new();
        for d in current_dead {
            if processed_deaths.insert(d) {
                if let Some(effect) = board.players[d].effective_role().on_death(d, &death_ctx) {
                    match effect {
                        DeathEffect::Kill {
                            seat: kill_seat,
                            announcement,
                        } => {
                            if !dead.contains(&kill_seat) && !hunter_shielded.contains(&kill_seat) {
                                newly_killed.push(kill_seat);
                                death_announcements.push(announcement);
                            }
                        }
                        DeathEffect::ForfeitWins(seats) => {
                            death_forfeits.extend(seats);
                        }
                    }
                }
                if board.marks[d] == Mark::Love {
                    for other in 0..n {
                        if other != d && board.marks[other] == Mark::Love && !dead.contains(&other)
                        {
                            newly_killed.push(other);
                            death_announcements.push(irc_format!(
                                "💔 {} died of heartbreak alongside their Lover!",
                                roster.seats[other].user.nick().bold().red(),
                            ));
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
    let mut reveal_lines = Vec::new();
    reveal_lines.push(irc_format!(
        "{} #1 {} | #2 {} | #3 {}",
        "🌅 DAWN REVEAL — Center:".bold(),
        board.center[0].display_reveal(),
        board.center[1].display_reveal(),
        board.center[2].display_reveal(),
    ));

    let mut role_items: Vec<Text> = Vec::with_capacity(n);
    for idx in 0..n {
        let mut token_suffix = String::new();
        if board.marks[idx] != Mark::Clarity {
            token_suffix.push_str(&format!(" [{}]", board.marks[idx].name()));
        }
        if let Some(art) = board.artifacts.get(&idx) {
            token_suffix.push_str(&format!(" {{{}}}", art.name()));
        }
        if dead.contains(&idx) {
            token_suffix.push_str(" 💀");
        }
        let nick = roster.seats[idx].user.nick();
        let item = if board.players[idx].face.id() == roster.seats[idx].initial_role.id() {
            irc_format!(
                "{}: {}{token_suffix}",
                nick.as_str().bold(),
                board.players[idx].display_reveal(),
            )
        } else {
            irc_format!(
                "{}: {} → {}{token_suffix}",
                nick.as_str().bold(),
                roster.seats[idx].initial_role.styled_name(),
                board.players[idx].display_reveal(),
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
        reveal_lines.push(prefix + Text::join(chunk, " | "));
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
        "💚 Nobody died!".green().into()
    } else {
        let mut sorted_dead: Vec<usize> = dead.iter().copied().collect();
        sorted_dead.sort_unstable();
        let dead_nicks = roster.format_indices(&sorted_dead);
        irc_format!("{} {}", "💀 Killed:".bold().red(), dead_nicks.as_str().bold())
    };

    room.say(irc_format!(
        "{} Votes: {tally_str} | {outcome_span}",
        "🌅 DAWN —".bold(),
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

    // 4a. Special role & artifact victories (`SpecialVictory` + `CudgelOfTheTanner`):
    let mut forced_team_winner: Option<Team> = None;
    let mut blocked_teams: HashSet<Team> = HashSet::new();
    let mut winners: HashMap<usize, &'static str> = HashMap::new();
    for seat in 0..n {
        if let Some(tok) = board.artifacts.get(&seat) {
            if tok.team_override().is_some() {
                if let Some((win_label, blocked)) = tok.check_independent_win(seat, &vctx) {
                    winners.insert(seat, win_label);
                    blocked_teams.extend(blocked.iter().copied());
                }
                continue;
            }
        }
        let role = board.players[seat].effective_role();
        match role.special_victory(seat, &vctx) {
            Some(SpecialVictory::ForceTeam { team, announcement })
                if forced_team_winner.is_none()
                    && board.effective_team(seat) == role.team() =>
            {
                room.say(announcement)?;
                forced_team_winner = Some(team);
            }
            Some(SpecialVictory::Independent { blocks })
                if board.effective_team(seat) == Team::Independent =>
            {
                winners.insert(seat, role.name());
                blocked_teams.extend(blocks.iter().copied());
            }
            _ => {}
        }
    }

    // 4b. Multi-faction victory evaluation (Village vs Werewolves/Evil vs Vampires):
    let wolf_threats: Vec<usize> = (0..n).filter(|&i| board.is_wolf_threat(i)).collect();
    let vamp_threats: Vec<usize> = (0..n).filter(|&i| board.is_vampire_threat(i)).collect();
    let minions_for = |team: Team| -> Vec<usize> {
        (0..n)
            .filter(|&i| {
                board.effective_team(i) == team && board.players[i].effective_role().is_minion()
            })
            .collect()
    };
    let wolf_minions = minions_for(Team::Werewolf);
    let vamp_minions = minions_for(Team::Vampire);

    let (village_wins, werewolf_wins, vampire_wins) = if let Some(forced) = forced_team_winner {
        (
            forced == Team::Village,
            forced == Team::Werewolf,
            forced == Team::Vampire,
        )
    } else {
        let any_monster_threats = !wolf_threats.is_empty() || !vamp_threats.is_empty();
        let any_monster_killed = wolf_threats.iter().any(|s| dead.contains(s))
            || vamp_threats.iter().any(|s| dead.contains(s));

        let v_win = if any_monster_threats {
            !blocked_teams.contains(&Team::Village) && any_monster_killed
        } else {
            !blocked_teams.contains(&Team::Village) && dead.is_empty()
        };

        let faction_wins = |team: Team, threats: &[usize], minions: &[usize]| -> bool {
            if blocked_teams.contains(&team) {
                return false;
            }
            if !threats.is_empty() {
                threats.iter().all(|s| !dead.contains(s))
            } else {
                !minions.is_empty()
                    && minions.iter().all(|m| !dead.contains(m))
                    && dead.iter().any(|&d| board.effective_team(d) != team)
            }
        };

        (
            v_win,
            faction_wins(Team::Werewolf, &wolf_threats, &wolf_minions),
            faction_wins(Team::Vampire, &vamp_threats, &vamp_minions),
        )
    };

    for (team_won, team, label) in [
        (village_wins, Team::Village, "Village"),
        (werewolf_wins, Team::Werewolf, "Werewolf Team"),
        (vampire_wins, Team::Vampire, "Vampire Team"),
    ] {
        if team_won {
            for seat in 0..n {
                if board.effective_team(seat) == team {
                    winners.insert(seat, label);
                }
            }
        }
    }

    // 4c. Individual win-forfeit penalties (`Mark::Disease`, `DeathEffect::ForfeitWins`, & `Artifact` conduct):
    for forfeited in death_forfeits {
        winners.remove(&forfeited);
    }
    let casemap = room.casemap();
    for seat in 0..n {
        if let Some(voted) = votes[seat] {
            if board.marks[voted] == Mark::Disease {
                winners.remove(&seat);
            }
        }
        if let Some(tok) = board.artifacts.get(&seat) {
            if let Some(forfeit_msg) =
                tok.check_day_conduct_forfeit(seat, roster, casemap, day_channel_log)
            {
                winners.remove(&seat);
                room.say(forfeit_msg)?;
            }
        }
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

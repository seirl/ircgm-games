//! **Voting** resolution, chain-kills, multi-faction victory evaluation, and deferred `!reveal` chronicle.

use std::collections::{HashMap, HashSet};

use ircgm::prelude::*;

use crate::board::{BoardCards, Card, Roster, SeatSet};
use crate::game::GameSummary;
use crate::marks::Mark;
use crate::role::{
    DeathCtx, DeathEffect, Role, SpecialVictory, Team, TeamBlock, VictoryCtx, VoteDefense,
    WinnerFilterCtx, WinnerMap,
};
use crate::roles::{
    daybreak::alpha_wolf::CenterWolfCard, super_villains::temptress::ExtraVillainCard,
};

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
    mut board: BoardCards,
    mut chronicle: Vec<Text>,
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

    // Apply vote-triggered role conversions (`Cursed`, `Windy Wendy`) before evaluating alignments & protections:
    let mut vote_conversions: Vec<(usize, &'static dyn Role, Text)> = Vec::new();
    for seat in 0..n {
        if !board.has_role_override_artifact(seat) && board.mark(seat) != Mark::Vampire {
            let role = board.player_card(seat).effective_role();
            if let Some((new_role, entry)) = role.vote_conversion(seat, roster, &board, &votes) {
                vote_conversions.push((seat, new_role, entry));
            }
        }
    }
    for (seat, new_role, entry) in vote_conversions {
        board.transform_player_on_vote(seat, new_role);
        chronicle.push(entry);
    }

    let max_raw_votes = tally.iter().copied().max().unwrap_or(0);
    let death_ctx = DeathCtx {
        roster,
        board: &board,
        votes: &votes,
        tally: &tally,
        max_votes: max_raw_votes,
    };

    // Determine active monster factions in front of players (`Epic Battle` when >= 2 monster factions clash):
    let threats_by_faction: Vec<(Team, SeatSet)> = Team::MONSTER_TEAMS
        .iter()
        .map(|&team| {
            let seats: SeatSet = (0..n)
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
    let has_non_monster = (0..n).any(|i| !Team::MONSTER_TEAMS.contains(&board.effective_team(i)));
    let is_epic_battle = present_monster_factions.len() >= 3
        || (present_monster_factions.len() == 2 && has_non_monster);

    // Collect unified vote defenses across each seat's active EndgameModifiers:
    let mut vote_immune = SeatSet::EMPTY;
    let mut hunter_shielded = SeatSet::EMPTY;
    let mut immunities: Vec<(usize, Text)> = Vec::new();
    let mut ricochets: Vec<(usize, usize, Text)> = Vec::new();
    for seat in 0..n {
        for modifier in board.seat_endgame_modifiers(seat) {
            match modifier.vote_defense(seat, &death_ctx) {
                Some(VoteDefense::Immune {
                    seat: protected_seat,
                    blocks_hunter_shot,
                    announcement,
                }) => {
                    vote_immune.insert(protected_seat);
                    if blocks_hunter_shot {
                        hunter_shielded.insert(protected_seat);
                    }
                    immunities.push((protected_seat, announcement));
                }
                Some(VoteDefense::Ricochet {
                    from,
                    to,
                    announcement,
                }) => {
                    ricochets.push((from, to, announcement));
                }
                None => {}
            }
        }
    }

    // Standard ONUW: highest vote count >= 2 among non-immune seats dies.
    // Epic Battle (>= 3 active factions, e.g. 2+ monster factions + Village): BOTH most votes (>= 2)
    // AND 2nd-most votes (>= 1) die, unless 2+ players tied for most votes (in which case all tied for 1st die).
    let killable_seats = roster.all_seats() - vote_immune;
    let max_killable_votes = killable_seats
        .into_iter()
        .map(|idx| tally[idx])
        .max()
        .unwrap_or(0);

    let mut saved_by_immunity = SeatSet::EMPTY;
    for &(protected_seat, _) in &immunities {
        let p_votes = tally[protected_seat];
        let saved_from_vote = (p_votes >= 2 && p_votes >= max_killable_votes)
            || (is_epic_battle && max_raw_votes >= 2 && p_votes >= 1 && p_votes >= max_killable_votes);
        if saved_from_vote {
            saved_by_immunity.insert(protected_seat);
        }
    }

    let mut dead = SeatSet::EMPTY;
    if max_killable_votes >= 2 {
        let top_tied: SeatSet = killable_seats
            .into_iter()
            .filter(|&idx| tally[idx] == max_killable_votes)
            .collect();
        dead |= top_tied;

        if is_epic_battle && top_tied.len() == 1 {
            let runners_up = killable_seats - top_tied;
            let second_votes = runners_up
                .into_iter()
                .map(|idx| tally[idx])
                .max()
                .unwrap_or(0);
            if second_votes >= 1 {
                for idx in runners_up {
                    if tally[idx] == second_votes {
                        dead.insert(idx);
                    }
                }
            }
        }
    } else if is_epic_battle && max_raw_votes >= 2 && max_killable_votes == 1 {
        // An immune seat absorbed the >= 2-vote top spot in an Epic Battle;
        // the unprotected 1-vote runner(s)-up still die!
        for idx in killable_seats {
            if tally[idx] == 1 {
                dead.insert(idx);
            }
        }
    }

    // Apply vote-execution ricochets (`Ricochet Rhino`) before `on_death` chain-kills:
    let mut ricochet_announcements: Vec<Text> = Vec::new();
    if !ricochets.is_empty() && !dead.is_empty() {
        let initial_vote_dead = dead;
        for s in initial_vote_dead {
            if ricochets.iter().any(|&(src, _, _)| src == s) {
                dead.remove(s);
                let mut cur = s;
                let mut visited = SeatSet::single(s);
                for _ in 0..n {
                    if let Some(&(_, dst, ref msg)) =
                        ricochets.iter().find(|&&(src, _, _)| src == cur)
                    {
                        ricochet_announcements.push(msg.clone());
                        if !visited.insert(dst) {
                            cur = s;
                            break;
                        }
                        cur = dst;
                    } else {
                        break;
                    }
                }
                if cur != s {
                    if vote_immune.contains(cur) {
                        saved_by_immunity.insert(cur);
                    } else {
                        dead.insert(cur);
                    }
                }
            }
        }
    }

    let mut death_announcements: Vec<Text> = ricochet_announcements;

    // Fixpoint loop over `EndgameModifier::on_death` (`Hunter`, `Mark::Love`, etc.):
    let mut processed_deaths = SeatSet::EMPTY;
    let mut chain_announcements: Vec<Text> = Vec::new();
    loop {
        let unprocessed = dead - processed_deaths;
        if unprocessed.is_empty() {
            break;
        }
        processed_deaths |= unprocessed;
        let mut newly_killed = SeatSet::EMPTY;
        for d in unprocessed {
            for modifier in board.seat_endgame_modifiers(d) {
                for effect in modifier.on_death(d, &death_ctx) {
                    let DeathEffect::Kill {
                        seat: kill_seat,
                        respects_protection,
                        announcement,
                    } = effect;
                    let blocked = respects_protection && hunter_shielded.contains(kill_seat);
                    if !(dead | newly_killed).contains(kill_seat) {
                        if blocked {
                            saved_by_immunity.insert(kill_seat);
                            chain_announcements.push(irc_format!(
                                "🏹 {} ({}) fired at {}, but {} could not be killed!",
                                roster.seats[d].user.nick().bold(),
                                modifier.modifier_name().bold().orange(),
                                roster.seats[kill_seat].user.nick().bold(),
                                roster.seats[kill_seat].user.nick().bold(),
                            ));
                        } else {
                            newly_killed.insert(kill_seat);
                            chain_announcements.push(announcement);
                        }
                    }
                }
            }
        }
        dead |= newly_killed;
    }

    for (protected_seat, announcement) in immunities {
        if saved_by_immunity.contains(protected_seat) {
            death_announcements.push(announcement);
        }
    }
    death_announcements.extend(chain_announcements);

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
        if dead.contains(idx) {
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
        let dead_nicks = roster.format_seats(dead);
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
        dead_seats: dead,
    };

    // 4a. Special victories across each seat's active EndgameModifiers:
    let mut forced_team_candidate: Option<(Team, Text)> = None;
    let mut team_block = TeamBlock::None;
    let mut hunted_seats = SeatSet::EMPTY;
    let mut hunted_label = "Oracle";
    let mut with_team_claims: Vec<(usize, Team, &'static str)> = Vec::new();
    let mut winners: WinnerMap = HashMap::new();
    for seat in 0..n {
        for modifier in board.seat_endgame_modifiers(seat) {
            for claim in modifier.victory_claims(seat, &vctx) {
                match claim {
                    SpecialVictory::ForceTeam { team, announcement }
                        if forced_team_candidate.is_none() =>
                    {
                        forced_team_candidate = Some((team, announcement));
                    }
                    SpecialVictory::Independent(block) => {
                        winners.insert(seat, modifier.modifier_name());
                        team_block = team_block.max(block);
                    }
                    SpecialVictory::WithTeam(team) => {
                        with_team_claims.push((seat, team, modifier.modifier_name()));
                    }
                    SpecialVictory::Hunted => {
                        hunted_seats.insert(seat);
                        hunted_label = modifier.modifier_name();
                    }
                    _ => {}
                }
            }
        }
    }

    let is_hunted_card = |c: Card| {
        matches!(
            c.effective_role().special_victory(0, &vctx),
            Some(SpecialVictory::Hunted)
        )
    };
    let center_hunted = (0..3).any(|c| is_hunted_card(board.center_card(c)))
        || is_hunted_card(board.get_key::<CenterWolfCard>().0)
        || is_hunted_card(board.get_key::<ExtraVillainCard>().0);
    if !hunted_seats.is_empty() || center_hunted {
        forced_team_candidate = None;
        team_block = TeamBlock::AllTeams;
        with_team_claims.clear();
        winners.clear();
        if hunted_seats.intersects(dead) {
            for seat in roster.all_seats() - hunted_seats {
                winners.insert(seat, "Oracle Slain");
            }
        } else {
            for seat in hunted_seats - dead {
                winners.insert(seat, hunted_label);
            }
        }
    }

    let forced_team_winner = match forced_team_candidate {
        Some((team, announcement)) if !team_block.blocks(team) => {
            room.say(announcement)?;
            Some(team)
        }
        _ => None,
    };

    // 4b. Multi-faction & Epic Battle victory evaluation (`Village` vs `Werewolf` / `Vampire` / `Alien` / `SuperVillain`):
    let threats_of = |team: Team| -> SeatSet {
        threats_by_faction
            .iter()
            .find(|(t, _)| *t == team)
            .map(|(_, seats)| *seats)
            .unwrap_or(SeatSet::EMPTY)
    };
    let minions_for = |team: Team| -> SeatSet {
        (0..n)
            .filter(|&i| board.effective_team(i) == team && !board.is_monster_threat_for(i, team))
            .collect()
    };

    let mut winning_teams: HashSet<Team> = HashSet::new();
    if let Some(forced) = forced_team_winner {
        winning_teams.insert(forced);
    } else {
        let village_wins = if !present_monster_factions.is_empty() {
            !team_block.blocks(Team::Village)
                && present_monster_factions
                    .iter()
                    .all(|&f| threats_of(f).intersects(dead))
        } else {
            !team_block.blocks(Team::Village) && dead.is_empty()
        };
        if village_wins {
            winning_teams.insert(Team::Village);
        }

        for &team in Team::MONSTER_TEAMS {
            if team_block.blocks(team) {
                continue;
            }
            let threats = threats_of(team);
            let minions = minions_for(team);
            if !threats.is_empty() {
                let all_own_survive = threats.is_disjoint(dead);
                let won = if present_monster_factions.len() >= 2 {
                    let rival_killed = present_monster_factions
                        .iter()
                        .any(|&other| other != team && threats_of(other).intersects(dead));
                    all_own_survive && rival_killed
                } else {
                    all_own_survive
                };
                if won {
                    winning_teams.insert(team);
                }
            } else {
                // Non-threat followers (`Minion`, `Squire`, `Oracle [Alien Team]`, monster-aligned `Nostradamus`)
                // with 0 primary threats in play win individually if they survive AND a non-follower dies:
                for m in minions - dead {
                    if !(dead - minions).is_empty() {
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
    for (seat, team, label) in with_team_claims {
        if winning_teams.contains(&team) {
            winners.insert(seat, label);
        }
    }

    // 4c. Win overrides (`Mark::Traitor`) followed by win-forfeit penalties (`Diseased`, `Mark::Disease`, `Artifact` conduct):
    let mut forfeit_announcements = Vec::new();
    {
        let mut fctx = WinnerFilterCtx {
            roster,
            board: &board,
            votes: &votes,
            dead_seats: dead,
            team_block,
            winning_teams: &winning_teams,
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

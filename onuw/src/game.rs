//! Thin single-game orchestrator wiring together `lobby`, `deck`, `night`, `day`, and `voting`.

use std::convert::Infallible;
use std::time::Duration;

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster, SeatSet};
use crate::voting::{resolve_and_announce_winners, GameReveal};
use crate::day::{run_day_phase, run_voting_phase};
use crate::deck::DeckSpec;
use crate::lobby::{recruit_players_tracked, send_active_game_help, send_single_role_info, LobbyPhaseTracker};
use crate::night::{format_role_order, run_night_phase, NightTimings};
use crate::roles::find_role;

pub const MIN_PLAYERS: usize = 3;
pub const MAX_PLAYERS: usize = 10;

#[derive(Clone, Debug)]
pub struct GameConfig {
    pub lobby_duration: Duration,
    pub night_timings: NightTimings,
    /// When `None`, Day duration scales as `(player_count + 2).clamp(5, 8)` minutes.
    pub day_duration_override: Option<Duration>,
    pub vote_timeout: Duration,
    pub vote_warn_remaining: Option<Duration>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            lobby_duration: Duration::from_secs(500),
            night_timings: NightTimings::default(),
            day_duration_override: None,
            vote_timeout: Duration::from_secs(45),
            vote_warn_remaining: Some(Duration::from_secs(10)),
        }
    }
}

impl GameConfig {
    pub fn fast_mode() -> Self {
        Self {
            lobby_duration: Duration::from_secs(30),
            night_timings: NightTimings {
                tier_budget: Duration::from_secs(15),
                copycat_pick_budget: Duration::from_secs(10),
                warn_remaining: Some(Duration::from_secs(5)),
            },
            day_duration_override: Some(Duration::from_secs(90)),
            vote_timeout: Duration::from_secs(25),
            vote_warn_remaining: Some(Duration::from_secs(5)),
        }
    }

    pub fn for_tests() -> Self {
        Self {
            lobby_duration: Duration::from_secs(15),
            night_timings: NightTimings::fast_for_tests(),
            day_duration_override: Some(Duration::from_secs(30)),
            vote_timeout: Duration::from_secs(10),
            vote_warn_remaining: None,
        }
    }

    pub fn day_duration(&self, player_count: usize) -> Duration {
        self.day_duration_override.unwrap_or_else(|| {
            let mins = (player_count + 2).clamp(5, 8) as u64;
            Duration::from_secs(mins * 60)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayOutcome {
    Completed(GameReveal),
    NotEnoughPlayers,
}

#[derive(Debug)]
pub struct GameSummary {
    pub final_board: BoardCards,
    pub chronicle: Vec<Text>,
    pub votes: Vec<Option<usize>>,
    pub dead_seats: SeatSet,
    pub winners: Vec<(usize, &'static str)>,
    pub reveal: GameReveal,
}

pub async fn run_single_game(
    bot: &Bot,
    room: &Room,
    host: User,
    initial_spec: DeckSpec,
    config: &GameConfig,
) -> Result<PlayOutcome> {
    run_single_game_tracked(bot, room, host, initial_spec, config, None).await
}

pub async fn run_single_game_tracked(
    bot: &Bot,
    room: &Room,
    host: User,
    initial_spec: DeckSpec,
    config: &GameConfig,
    phase_tracker: Option<&LobbyPhaseTracker>,
) -> Result<PlayOutcome> {
    let Some((players, deck_spec)) =
        recruit_players_tracked(room, host, initial_spec, config, phase_tracker).await?
    else {
        return Ok(PlayOutcome::NotEnoughPlayers);
    };

    let (roster, initial_board) = deck_spec
        .deal(&players)
        .map_err(|_| Error::InvalidInput("invalid ONUW deck size"))?;

    let summary = run_dealt_game(
        bot,
        room,
        &roster,
        initial_board,
        &deck_spec.mode_label(),
        config,
    )
    .await?;
    Ok(PlayOutcome::Completed(summary.reveal))
}

/// Runs a game from an already-dealt [`Roster`] and [`BoardCards`].
pub async fn run_dealt_game(
    bot: &Bot,
    room: &Room,
    roster: &Roster,
    initial_board: BoardCards,
    mode_label: &str,
    config: &GameConfig,
) -> Result<GameSummary> {
    let players: Vec<User> = roster.seats.iter().map(|s| s.user.clone()).collect();
    let mut mod_guard = room.moderation_guard().ok();

    // Phase 1: Announce deck, seating ring, and deal secret role cards in DM:
    room.say(irc_format!(
        "🃏 {} {} {} ({} cards): {}",
        mode_label.bold(),
        "—".grey(),
        "Shuffling deck".bold(),
        roster.deck.len(),
        roster.format_deck_styled(),
    ))?;
    room.say(irc_format!(
        "👥 {}: {}",
        "Table circle".bold(),
        roster.format_table_circle()
    ))?;

    for seat in &roster.seats {
        let summary = seat.initial_role.summary_for_deck(roster);
        let _ = seat.user.say(irc_format!(
            "🔒 Your initial card is: {} — {summary}",
            seat.initial_role.styled_name(),
        ));
    }

    let deck_snapshot = roster.format_deck_styled();
    let deck_len = roster.deck.len();
    let order_snapshot = format_role_order(&roster.deck);
    let info_responder = bot.inbox().on_map(move |msg| {
        if !(msg.is_in(room) || msg.is_dm()) {
            return None::<Infallible>;
        }
        if msg.is_cmd("!deck") {
            let _ = msg.reply(irc_format!(
                "🃏 {} {}",
                format!("Deck ({deck_len} cards):").bold(),
                deck_snapshot.clone(),
            ));
        } else if msg.is_cmd("!order") {
            let _ = msg.reply(order_snapshot.clone());
        } else if msg.is_cmd("!players") || msg.is_cmd("!who") {
            let _ = msg.reply(irc_format!(
                "👥 {}: {}",
                "Table circle".bold(),
                roster.format_table_circle(),
            ));
        } else if msg.is_cmd("!help") {
            let _ = send_active_game_help(msg);
        } else if let Some(arg) = msg.cmd_arg("!role").or_else(|| msg.cmd_arg("!info")) {
            if let Some(r) = find_role(arg) {
                let _ = send_single_role_info(msg, r);
            }
        }
        None::<Infallible>
    });

    let game_flow = async {
        // Phase 2: Fixed-Clock Dusk + Night Scheduler
        if let Some(g) = &mut mod_guard {
            let _ = g.set_moderated(true);
            let _ = g.devoice_many(&players);
            let _ = g.set_topic("ONUW — 🌙 Dusk & Night Phase");
        }
        let (final_board, chronicle) =
            run_night_phase(bot, room, roster, initial_board, &config.night_timings).await?;

        // Phase 3: Timed Day Discussion (open one channel inbox across Day + Voting for Artifact conduct rules)
        let mut day_channel_log: Vec<ChatMessage> = Vec::new();
        let mut conduct_inbox = room.inbox().lossy();
        if let Some(g) = &mut mod_guard {
            let _ = g.set_moderated(true);
            let _ = g.voice_many(&players);
            let _ = g.set_topic("ONUW — ☀️ Day Phase");
        }
        run_day_phase(
            room,
            roster,
            &final_board,
            config.day_duration(roster.len()),
        )
        .await?;
        while let Ok(Some(msg)) = conduct_inbox.try_next_chat() {
            day_channel_log.push(msg);
        }

        // Phase 4: Public Channel Voting (`!vote <nick>` in channel; players stay voiced!)
        if let Some(g) = &mut mod_guard {
            let _ = g.set_topic("ONUW — 🗳️ Voting Phase");
        }
        let votes = run_voting_phase(
            bot,
            room,
            roster,
            config.vote_timeout,
            config.vote_warn_remaining,
        )
        .await?;
        while let Ok(Some(msg)) = conduct_inbox.try_next_chat() {
            day_channel_log.push(msg);
        }

        // Phase 5: Voting Results & Victory Resolution (full chronicle deferred to `!reveal`)
        if let Some(g) = mod_guard.take() {
            let _ = g.restore();
        }
        resolve_and_announce_winners(room, roster, final_board, chronicle, votes, &day_channel_log)
    };

    let Outcome::Completed(res) = game_flow.until(info_responder).await;
    res
}

#[cfg(test)]
#[path = "game_test.rs"]
mod test;

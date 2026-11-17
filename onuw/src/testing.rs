//! Lightweight deterministic test harness for unit-testing individual ONUW [`Role`]s,
//! fixed-clock Dusk/Night tier scheduling, timing-attack invariance, and Voting victory resolution.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use ircgm::prelude::*;
use ircgm::testing::{test_harness, TestServer};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::artifacts::Artifact;
use crate::board::{BoardCards, BoardKey, Card, Roster, SeatInfo};
use crate::voting::resolve_and_announce_winners;
use crate::game::GameSummary;
use crate::marks::Mark;
use crate::night::{run_night_phase, NightTimings};
use crate::role::Role;

pub const TEST_ROOM: &str = "#onuw";

fn looks_like_prompt(plain: &str) -> bool {
    let lower = plain.to_ascii_lowercase();
    lower.contains("reply ")
        || lower.contains("pick ")
        || lower.contains("usage:")
        || lower.contains("whisper here")
        || lower.contains("give center")
        || lower.contains("you must ")
        || lower.contains("tune cosmic frequencies")
        || lower.contains("point at ")
}

/// Ergonomic test fixture wrapping [`ircgm::testing::test_harness`] for a specific seat + center deal.
pub struct OnuwTestHarness {
    pub bot: Bot,
    pub srv: TestServer,
    pub room: Room,
    pub roster: Roster,
    pub initial_board: BoardCards,
    pub timings: NightTimings,
    night_origin: Option<Instant>,
    collected_dms: HashMap<String, Vec<String>>,
    pub timed_dms: HashMap<String, Vec<(Duration, String)>>,
    pub timed_channel: Vec<(Duration, String)>,
    pub day_channel_log: Vec<ChatMessage>,
}

impl OnuwTestHarness {
    /// Creates an in-memory IRC session joined to `#onuw` with exact `player_deals` and `center` cards.
    pub async fn new(
        player_deals: &[(&str, &'static dyn Role)],
        center: [&'static dyn Role; 3],
    ) -> Self {
        let (bot, srv) = test_harness("GM");
        let nicks: Vec<&str> = player_deals.iter().map(|(n, _)| *n).collect();
        srv.complete_join(TEST_ROOM, &nicks);
        srv.settle().await;

        let room = bot.room(TEST_ROOM).expect("room handle");
        let mut seats = Vec::with_capacity(player_deals.len());
        let mut player_cards = Vec::with_capacity(player_deals.len());
        let mut deck = Vec::with_capacity(player_deals.len() + 3);

        for &(nick, role) in player_deals {
            let user = bot.user(nick).expect("user handle");
            seats.push(SeatInfo {
                user,
                initial_role: role,
            });
            player_cards.push(Card::new(role));
            deck.push(role);
        }
        let center_cards = [
            Card::new(center[0]),
            Card::new(center[1]),
            Card::new(center[2]),
        ];
        deck.extend_from_slice(&center);

        let roster = Roster { seats, deck };
        let initial_board = BoardCards::new(player_cards, center_cards);

        Self {
            bot,
            srv,
            room,
            roster,
            initial_board,
            timings: NightTimings::fast_for_tests(),
            night_origin: None,
            collected_dms: HashMap::new(),
            timed_dms: HashMap::new(),
            timed_channel: Vec::new(),
            day_channel_log: Vec::new(),
        }
    }

    fn elapsed_since_origin(&self) -> Duration {
        self.night_origin
            .map(|t0| Instant::now().saturating_duration_since(t0))
            .unwrap_or(Duration::ZERO)
    }

    fn drain_channel_with_timestamps(&mut self) {
        let ts = self.elapsed_since_origin();
        for msg in self.srv.take_messages_to(TEST_ROOM) {
            self.timed_channel.push((ts, msg));
        }
    }

    fn pull_unread_batch(&mut self, nick: &str) -> Vec<(Duration, String)> {
        let ts = self.elapsed_since_origin();
        self.srv
            .take_messages_to(nick)
            .into_iter()
            .map(|m| (ts, m))
            .collect()
    }

    fn record_read_batch(&mut self, nick: &str, batch: Vec<(Duration, String)>) -> String {
        let mut lines = Vec::with_capacity(batch.len());
        for (ts, m) in batch {
            lines.push(m.clone());
            self.timed_dms
                .entry(nick.to_owned())
                .or_default()
                .push((ts, m.clone()));
            self.collected_dms
                .entry(nick.to_owned())
                .or_default()
                .push(m);
        }
        lines.join("\n")
    }

    /// Spawns [`run_night_phase`] in a background task and records `t = 0`.
    pub fn spawn_night(&mut self) -> NightRunHandle {
        self.night_origin = Some(Instant::now());
        let room = self.room.clone();
        let seats = self.roster.seats.clone();
        let deck = self.roster.deck.clone();
        let board = self.initial_board.clone();
        let timings = self.timings.clone();

        let join = tokio::spawn(async move {
            let roster = Roster { seats, deck };
            run_night_phase(&room, &roster, board, &timings).await
        });
        NightRunHandle { join }
    }

    /// Waits until `nick` receives an interactive prompt DM (stepping across fixed-clock tier
    /// boundaries automatically when needed), then sends `reply` in DM.
    pub async fn reply_dm(&mut self, nick: &str, reply: &str) {
        self.wait_for_prompt_dm(nick).await;
        self.srv.dm(nick, reply);
        self.srv.settle().await;
        self.drain_channel_with_timestamps();
    }

    /// Sends a DM from `nick` without waiting for a new prompt (e.g. silent-pointing updates or follow-up replies).
    pub async fn send_dm(&mut self, nick: &str, text: &str) {
        self.srv.dm(nick, text);
        self.srv.settle().await;
        self.drain_channel_with_timestamps();
    }

    /// Waits until `nick` receives an interactive prompt DM (`Reply ...`, `Pick ...`, etc.),
    /// recording any non-prompt informational lines (`Fellow Werewolf pack`, confirmation echoes)
    /// along the way without mistaking them for the prompt itself!
    pub async fn wait_for_prompt_dm(&mut self, nick: &str) -> String {
        self.wait_for_dm_matching(nick, looks_like_prompt).await
    }

    /// Waits until `nick` receives ANY new DM line (prompt, rejection, or notification).
    pub async fn wait_for_new_dm(&mut self, nick: &str) -> String {
        self.wait_for_dm_matching(nick, |_| true).await
    }

    async fn wait_for_dm_matching(&mut self, nick: &str, pred: impl Fn(&str) -> bool) -> String {
        loop {
            let first = self.srv.recv_message_to(nick).await;
            self.srv.settle().await;
            self.drain_channel_with_timestamps();
            let mut batch = vec![(self.elapsed_since_origin(), first)];
            batch.extend(self.pull_unread_batch(nick));
            let matched = batch.iter().any(|(_, m)| pred(m));
            let joined = self.record_read_batch(nick, batch);
            if matched {
                return joined;
            }
        }
    }

    /// Advances virtual Tokio time (`#[tokio::test(start_paused = true)]`).
    pub async fn advance_time(&mut self, dur: Duration) {
        self.srv.advance(dur).await;
        self.drain_channel_with_timestamps();
    }

    /// Steps virtual time until `night` completes, recording exact arrival timestamps for all
    /// channel announcements and DMs!
    pub async fn finish_night(&mut self, night: NightRunHandle) -> NightReport {
        for _ in 0..8 {
            self.srv.settle().await;
            self.drain_channel_with_timestamps();
            if night.join.is_finished() {
                break;
            }
            tokio::task::yield_now().await;
        }
        while !night.join.is_finished() {
            tokio::time::sleep(Duration::from_millis(50)).await;
            self.srv.settle().await;
            self.drain_channel_with_timestamps();
        }

        let (board, chronicle) = night
            .join
            .await
            .expect("night task panicked")
            .expect("night DAG returned Err");
        let total_elapsed = self.elapsed_since_origin();
        self.srv.settle().await;
        self.drain_channel_with_timestamps();

        let nicks: Vec<String> = self.roster.seats.iter().map(|s| s.user.nick()).collect();
        for nick in nicks {
            let remaining = self.pull_unread_batch(&nick);
            if !remaining.is_empty() {
                let _ = self.record_read_batch(&nick, remaining);
            }
        }

        let dms_by_nick = std::mem::take(&mut self.collected_dms);
        let timed_dms = std::mem::take(&mut self.timed_dms);
        let timed_channel = std::mem::take(&mut self.timed_channel);
        let channel_lines: Vec<String> = timed_channel.iter().map(|(_, s)| s.clone()).collect();
        let chronicle_plain: Vec<String> = chronicle.iter().map(Text::plain).collect();

        NightReport {
            board,
            chronicle,
            chronicle_plain,
            dms_by_nick,
            timed_dms,
            timed_channel,
            channel_lines,
            total_elapsed,
        }
    }

    /// Simulates a player sending a public channel message in `#onuw` during Day/Voting.
    pub async fn day_chat(&mut self, nick: &str, text: &str) {
        let mut inbox = self.room.inbox();
        self.srv.chat(nick, TEST_ROOM, text);
        self.srv.settle().await;
        let msg = inbox.next_chat().await.expect("day_chat message");
        self.day_channel_log.push(msg);
    }

    /// Simulates a player sending a public `/me` (`CTCP ACTION`) in `#onuw` during Day/Voting.
    pub async fn day_action(&mut self, nick: &str, action_text: &str) {
        let mut inbox = self.room.inbox();
        self.srv.action(nick, TEST_ROOM, action_text);
        self.srv.settle().await;
        let msg = inbox.next_chat().await.expect("day_action message");
        self.day_channel_log.push(msg);
    }

    /// Runs Voting death/victory resolution on `night_report` with explicit `(voter_nick, Option<target_nick>)` votes.
    pub async fn resolve_voting(
        &mut self,
        night_report: NightReport,
        ballots: &[(&str, Option<&str>)],
    ) -> VotingReport {
        let mut votes = vec![None; self.roster.len()];
        for &(voter, target_opt) in ballots {
            let v_idx = self
                .roster
                .resolve_nick(voter)
                .unwrap_or_else(|| panic!("unknown voter `{voter}`"));
            votes[v_idx] = target_opt.map(|t| {
                self.roster
                    .resolve_nick(t)
                    .unwrap_or_else(|| panic!("unknown target `{t}`"))
            });
        }

        let summary = resolve_and_announce_winners(
            &self.room,
            &self.roster,
            night_report.board,
            night_report.chronicle,
            votes,
            &self.day_channel_log,
        )
        .expect("resolve_and_announce_winners failed");
        self.srv.settle().await;
        let channel_lines = self.srv.take_messages_to(TEST_ROOM);

        VotingReport {
            summary,
            channel_lines,
        }
    }

    pub fn seat_of(&self, nick: &str) -> usize {
        self.roster
            .resolve_nick(nick)
            .unwrap_or_else(|| panic!("unknown player `{nick}`"))
    }

    pub fn set_key<K: BoardKey>(&mut self, val: K) {
        self.initial_board.set_key_for_test(val);
    }
}

pub struct NightRunHandle {
    join: JoinHandle<Result<(BoardCards, Vec<Text>)>>,
}

pub struct NightReport {
    pub board: BoardCards,
    pub chronicle: Vec<Text>,
    pub chronicle_plain: Vec<String>,
    pub dms_by_nick: HashMap<String, Vec<String>>,
    pub timed_dms: HashMap<String, Vec<(Duration, String)>>,
    pub timed_channel: Vec<(Duration, String)>,
    pub channel_lines: Vec<String>,
    pub total_elapsed: Duration,
}

impl NightReport {
    pub fn effective_role_at(&self, roster: &Roster, nick: &str) -> &'static str {
        let idx = roster.resolve_nick(nick).expect("player exists");
        self.board.player_card(idx).effective_role().name()
    }

    pub fn face_role_at(&self, roster: &Roster, nick: &str) -> &'static str {
        let idx = roster.resolve_nick(nick).expect("player exists");
        self.board.player_card(idx).face_role().name()
    }

    pub fn center_role(&self, one_based: usize) -> &'static str {
        self.board.center_card(one_based - 1).effective_role().name()
    }

    pub fn mark_at(&self, roster: &Roster, nick: &str) -> Mark {
        let idx = roster.resolve_nick(nick).expect("player exists");
        self.board.mark(idx)
    }

    pub fn artifact_at(&self, roster: &Roster, nick: &str) -> Option<Artifact> {
        let idx = roster.resolve_nick(nick).expect("player exists");
        self.board.artifact(idx)
    }

    pub fn dms_to(&self, nick: &str) -> String {
        self.dms_by_nick
            .get(nick)
            .map(|v| v.join("\n"))
            .unwrap_or_default()
    }

    pub fn has_chronicle_substr(&self, needle: &str) -> bool {
        self.chronicle_plain.iter().any(|line| line.contains(needle))
    }

    pub fn full_chronicle_text(&self) -> String {
        self.chronicle_plain.join("\n")
    }
}

pub struct VotingReport {
    pub summary: GameSummary,
    pub channel_lines: Vec<String>,
}

impl VotingReport {
    pub fn winner_nicks(&self, roster: &Roster) -> Vec<String> {
        self.summary
            .winners
            .iter()
            .map(|&(idx, _)| roster.seats[idx].user.nick())
            .collect()
    }

    pub fn dead_nicks(&self, roster: &Roster) -> HashSet<String> {
        self.summary
            .dead_seats
            .into_iter()
            .map(|idx| roster.seats[idx].user.nick())
            .collect()
    }

    pub fn channel_text(&self) -> String {
        self.channel_lines.join("\n")
    }

    pub fn reveal_text(&self) -> String {
        self.summary
            .reveal
            .lines()
            .iter()
            .map(Text::plain)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

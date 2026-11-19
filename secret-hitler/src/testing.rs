//! Deterministic in-memory IRC test harness for **Secret Hitler** (`ShTestHarness`).

use ircgm::prelude::*;
use ircgm::testing::{test_harness, TestServer};
use tokio::task::JoinHandle;

use crate::game::{run_dealt_game, GameConfig, GameSetup, GameSummary};
use crate::state::{Policy, PolicyDeck, Role, SeatInfo};

pub const TEST_ROOM: &str = "#sh";

/// Ergonomic test fixture wrapping [`ircgm::testing::test_harness`] for deterministic
/// Secret Hitler deals (`seats`, `deck`, `initial_president`).
pub struct ShTestHarness {
    pub bot: Bot,
    pub srv: TestServer,
    pub room: Room,
    pub setup: GameSetup,
    pub config: GameConfig,
}

impl ShTestHarness {
    pub async fn new(
        seat_deals: &[(&str, Role)],
        deck_order: &[Policy],
        initial_president: usize,
    ) -> Self {
        let (bot, srv) = test_harness("GM");
        let nicks: Vec<&str> = seat_deals.iter().map(|(n, _)| *n).collect();
        srv.complete_join(TEST_ROOM, &nicks);
        srv.settle().await;

        let room = bot.room(TEST_ROOM).expect("room handle");
        let seats: Vec<SeatInfo> = seat_deals
            .iter()
            .map(|&(nick, role)| SeatInfo {
                user: bot.user(nick).expect("user handle"),
                role,
            })
            .collect();

        let setup = GameSetup::custom(
            seats,
            PolicyDeck::from_draw_order(deck_order.iter().copied()),
            initial_president,
        );

        Self {
            bot,
            srv,
            room,
            setup,
            config: GameConfig::for_tests(),
        }
    }

    pub fn spawn_game(&self) -> JoinHandle<Result<GameSummary>> {
        let bot = self.bot.clone();
        let room = self.room.clone();
        let setup = self.setup.clone();
        let cfg = self.config.clone();
        tokio::spawn(async move { run_dealt_game(&bot, &room, setup, &cfg).await })
    }

    pub async fn chat(&mut self, nick: &str, text: &str) {
        self.srv.chat(nick, TEST_ROOM, text);
        self.srv.settle().await;
    }

    pub async fn dm(&mut self, nick: &str, text: &str) {
        self.srv.dm(nick, text);
        self.srv.settle().await;
    }

    pub async fn vote_all(&mut self, ballots: &[(&str, &str)]) {
        for &(nick, choice) in ballots {
            self.srv.dm(nick, choice);
        }
        self.srv.settle().await;
    }

    pub fn take_channel_plain(&mut self) -> String {
        self.srv
            .take_messages_to(TEST_ROOM)
            .into_iter()
            .map(|s| Text::parse(&s).plain())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn take_channel_raw(&mut self) -> String {
        self.srv.take_messages_to(TEST_ROOM).join("\n")
    }

    pub fn take_dm_plain(&mut self, nick: &str) -> String {
        self.srv
            .take_messages_to(nick)
            .into_iter()
            .map(|s| Text::parse(&s).plain())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

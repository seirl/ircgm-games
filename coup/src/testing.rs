//! Shared deterministic test harness for **Coup** integration tests.

use ircgm::prelude::*;
use ircgm::testing::{test_harness, TestServer};

use crate::game::{run_dealt_game, GameConfig, GameSetup, GameSummary};
use crate::state::{Allegiance, BoardState, GameMode, PlayerState, Role};

pub const TEST_ROOM: &str = "#coup";

pub struct CoupTestHarness {
    pub bot: Bot,
    pub srv: TestServer,
    pub room: Room,
    pub setup: GameSetup,
    pub config: GameConfig,
}

impl CoupTestHarness {
    pub async fn new(
        mode: GameMode,
        players_spec: &[(&str, [Role; 2], u32, Option<Allegiance>)],
        deck: &[Role],
        treasury: u32,
        initial_turn: usize,
    ) -> Self {
        let (bot, srv) = test_harness("GM");
        let nicks: Vec<&str> = players_spec.iter().map(|(n, _, _, _)| *n).collect();
        srv.complete_join(TEST_ROOM, &nicks);
        srv.settle().await;

        let room = bot.room(TEST_ROOM).expect("joined test room");
        let users: Vec<User> = nicks
            .iter()
            .map(|n| bot.user(n).expect("user in room"))
            .collect();
        let states: Vec<PlayerState> = players_spec
            .iter()
            .map(|&(_, cards, coins, allegiance)| PlayerState::new(cards, coins, allegiance))
            .collect();
        let board = BoardState::new(mode, states, deck.to_vec(), treasury, initial_turn);
        let setup = GameSetup::custom(users, board);

        Self {
            bot,
            srv,
            room,
            setup,
            config: GameConfig::for_tests(),
        }
    }

    pub fn spawn_game(&self) -> tokio::task::JoinHandle<Result<GameSummary>> {
        let bot = self.bot.clone();
        let room = self.room.clone();
        let setup = self.setup.clone();
        let config = self.config.clone();
        tokio::spawn(async move { run_dealt_game(&bot, &room, setup, &config).await })
    }

    pub async fn pass_all(&mut self, nicks: &[&str]) {
        for &n in nicks {
            self.srv.chat(n, TEST_ROOM, "!pass");
        }
        self.srv.settle().await;
    }

    pub fn drain_room_plain(&mut self) -> String {
        Text::parse(&self.srv.take_messages_to(TEST_ROOM).join("\n")).plain()
    }

    pub fn drain_dm_plain(&mut self, nick: &str) -> String {
        Text::parse(&self.srv.take_messages_to(nick).join("\n")).plain()
    }
}

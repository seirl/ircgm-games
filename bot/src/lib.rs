//! Library surface for `ircgm-games-bot` (CLI, game registry, and lobby supervisor).

#![forbid(unsafe_code)]

pub mod cli;
pub mod games;
pub mod lobby;

pub use cli::{parse_cli, BotCli, ParseOutcome};
pub use games::EnabledGames;
pub use lobby::{admin_quit_loop, run_lobby};

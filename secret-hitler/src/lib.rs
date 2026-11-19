//! **Secret Hitler** (5–10 players, official rules) IRC game master bot built on [`ircgm`].

#![forbid(unsafe_code)]

pub mod game;
pub mod lobby;
pub mod state;
#[cfg(test)]
pub mod testing;

pub use game::{
    format_secret_role_dm, run_dealt_game, run_single_game, GameConfig, GameReveal, GameSetup,
    GameSummary, PlayOutcome,
};
pub use lobby::{
    handle_catalog_query, recruit_players, run_lobby_bot, send_lobby_help, SecretHitlerLaunch,
    SecretHitlerLobby, SecretHitlerRound,
};
pub use state::{
    exec_power, fascist_slot_powers, format_policies, format_track_reference,
    hitler_knows_fascists, role_distribution, BoardState, ChancellorChoice, ExecPower, Party,
    Policy, PolicyDeck, Role, Roster, SeatInfo, WinReason, MAX_PLAYERS, MIN_PLAYERS,
};

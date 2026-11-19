//! **Coup** + **Inquisitor** & **Reformation** expansion (2–10 players, official rules) IRC game
//! master bot built on [`ircgm`].

#![forbid(unsafe_code)]

pub mod actions;
pub mod game;
pub mod lobby;
pub mod state;
#[cfg(test)]
pub mod testing;

pub use actions::{
    parse_action, parse_block_role, parse_examine_decision, parse_exchange_keep,
    parse_influence_choice, Action, ActionSpec, Claim, ExamineDecision,
};
pub use game::{
    format_private_hand_dm, run_dealt_game, run_single_game, GameConfig, GameReveal, GameSetup,
    GameSummary, PlayOutcome,
};
pub use lobby::{
    handle_catalog_query, recruit_players, run_lobby_bot, send_lobby_help, CoupLaunch, CoupLobby,
    CoupRound,
};
pub use state::{
    copies_per_role, parse_role, Allegiance, BoardState, CardSlot, GameMode, PlayerState, Role,
    Roster, MAX_PLAYERS, MIN_PLAYERS,
};

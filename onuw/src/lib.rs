//! **One Night Ultimate Werewolf** IRC game master bot built on [`ircgm`].
//!
//! Covers all five One Night Ultimate games (`src/roles/{base,daybreak,vampire,alien,super_villains}/`)
//! and 59 expansion-ordered scenarios (`src/scenarios/`), with fixed-clock anti-timing-attack Night
//! scheduling (`night.rs`), public channel voting (`day.rs`), and vote/victory resolution (`voting.rs`).

#![forbid(unsafe_code)]

pub mod artifacts;
pub mod board;
pub mod day;
pub mod deck;
pub mod game;
pub mod lobby;
pub mod marks;
pub mod night;
pub mod role;
pub mod roles;
pub mod scenarios;
pub mod seat_set;
#[cfg(test)]
pub mod testing;
pub mod voting;

pub use artifacts::{Artifact, DayConductTracker};
pub use board::{BoardAccess, BoardCards, BoardKey, BoardMut, Card, Roster, SeatInfo};
pub use seat_set::{SeatIter, SeatSet};
pub use day::{run_day_phase, run_voting_phase};
pub use deck::{DeckBase, DeckSpec};
pub use game::{
    run_dealt_game, run_single_game, run_single_game_tracked, GameConfig, GameSummary, PlayOutcome,
    MAX_PLAYERS, MIN_PLAYERS,
};
pub use lobby::{recruit_players, recruit_players_tracked, run_lobby_bot, LobbyPhaseTracker};
pub use marks::Mark;
pub use night::{
    build_deck_schedule, format_role_order, run_night_phase, NightTimings, ScheduledTier, TierKind,
    TierPhase,
};
pub use role::{
    CopyTiming, DeathEffect, EffectiveAlignment, EndgameModifier, GameSet, GroupEffect,
    NightFaction, Phase, Role, RoleInfo, SeatEffect, SpecialVictory, Team, WakeStep,
};
pub use roles::{find_role, ALL_ROLES};
pub use scenarios::{
    all_scenarios, find_scenario, scenario_count, scenarios_for_player_count, scenarios_in_pack,
    Difficulty, Scenario, ScenarioPack, ScenarioRule,
};
pub use voting::{resolve_and_announce_winners, GameReveal};

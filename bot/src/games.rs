//! Game registry and bot-local `GameLobby` trait for `ircgm-games-bot`.

use std::future::Future;

use ircgm::prelude::*;
use ircgm_onuw::{GameConfig as OnuwGameConfig, OnuwLaunch, OnuwLobby, OnuwRound, ALL_ROLES};
use ircgm_secret_hitler::{
    GameConfig as SecretHitlerGameConfig, SecretHitlerLaunch, SecretHitlerLobby, SecretHitlerRound,
};

/// Bot-local three-phase lobby contract implemented for each registered game.
pub trait GameLobby: Send {
    type Launch: Send;
    type Round: Send;

    fn wait_for_start(
        &mut self,
        lobby: &Room,
    ) -> impl Future<Output = Result<Self::Launch>> + Send;

    fn recruit(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        launch: Self::Launch,
    ) -> impl Future<Output = Result<Option<Self::Round>>> + Send;

    fn play_round(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        round: Self::Round,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl GameLobby for OnuwLobby {
    type Launch = OnuwLaunch;
    type Round = OnuwRound;

    fn wait_for_start(
        &mut self,
        lobby: &Room,
    ) -> impl Future<Output = Result<Self::Launch>> + Send {
        OnuwLobby::wait_for_start(self, lobby)
    }

    fn recruit(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        launch: Self::Launch,
    ) -> impl Future<Output = Result<Option<Self::Round>>> + Send {
        OnuwLobby::recruit(self, bot, lobby, launch)
    }

    fn play_round(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        round: Self::Round,
    ) -> impl Future<Output = Result<()>> + Send {
        OnuwLobby::play_round(self, bot, lobby, round)
    }
}

impl GameLobby for SecretHitlerLobby {
    type Launch = SecretHitlerLaunch;
    type Round = SecretHitlerRound;

    fn wait_for_start(
        &mut self,
        lobby: &Room,
    ) -> impl Future<Output = Result<Self::Launch>> + Send {
        SecretHitlerLobby::wait_for_start(self, lobby)
    }

    fn recruit(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        launch: Self::Launch,
    ) -> impl Future<Output = Result<Option<Self::Round>>> + Send {
        SecretHitlerLobby::recruit(self, bot, lobby, launch)
    }

    fn play_round(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        round: Self::Round,
    ) -> impl Future<Output = Result<()>> + Send {
        SecretHitlerLobby::play_round(self, bot, lobby, round)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameKind {
    Onuw,
    SecretHitler,
}

pub struct GameEntry {
    pub slug: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
    pub kind: GameKind,
}

pub fn default_onuw_lobby() -> OnuwLobby {
    OnuwLobby::new(OnuwGameConfig::default())
}

pub fn default_secret_hitler_lobby() -> SecretHitlerLobby {
    SecretHitlerLobby::new(SecretHitlerGameConfig::default())
}

pub static REGISTRY: &[GameEntry] = &[
    GameEntry {
        slug: "onuw",
        aliases: &["werewolf"],
        summary: "One Night Ultimate Werewolf",
        kind: GameKind::Onuw,
    },
    GameEntry {
        slug: "secret_hitler",
        aliases: &["sh", "hitler", "secrethitler"],
        summary: "Secret Hitler (5–10p)",
        kind: GameKind::SecretHitler,
    },
];

pub fn find_entry(name: &str) -> Option<&'static GameEntry> {
    let word = name.split_whitespace().next()?;
    let norm = word
        .trim_start_matches('!')
        .to_ascii_lowercase()
        .replace('-', "_");
    REGISTRY
        .iter()
        .find(|g| g.slug == norm || g.aliases.contains(&norm.as_str()))
}

fn available_slug_list() -> String {
    REGISTRY
        .iter()
        .map(|g| g.slug)
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnabledGames {
    slugs: Vec<&'static str>,
}

impl EnabledGames {
    pub fn all() -> Self {
        Self {
            slugs: REGISTRY.iter().map(|g| g.slug).collect(),
        }
    }

    pub fn from_specs<S: AsRef<str>>(specs: &[S]) -> std::result::Result<Self, String> {
        if specs.is_empty() {
            return Ok(Self::all());
        }

        let mut selected: Vec<&'static str> = Vec::new();
        let mut saw_token = false;

        for spec in specs {
            for raw in spec.as_ref().split(',') {
                let token = raw.trim();
                if token.is_empty() {
                    continue;
                }
                saw_token = true;
                let norm = token.to_ascii_lowercase().replace('-', "_");
                if norm == "all" {
                    return Ok(Self::all());
                }
                if let Some(entry) = find_entry(&norm) {
                    if !selected.contains(&entry.slug) {
                        selected.push(entry.slug);
                    }
                } else {
                    return Err(format!(
                        "unknown game `{token}` (available: {})",
                        available_slug_list()
                    ));
                }
            }
        }

        if !saw_token || selected.is_empty() {
            return Err(format!(
                "--games must enable at least one game (available: {})",
                available_slug_list()
            ));
        }

        let ordered = REGISTRY
            .iter()
            .map(|g| g.slug)
            .filter(|slug| selected.contains(slug))
            .collect();

        Ok(Self { slugs: ordered })
    }

    pub fn slugs(&self) -> &[&'static str] {
        &self.slugs
    }

    pub fn slug_list(&self) -> String {
        self.slugs.join(", ")
    }

    pub fn is_enabled(&self, slug: &str) -> bool {
        self.slugs.contains(&slug)
    }

    pub fn entries(&self) -> impl Iterator<Item = &'static GameEntry> + '_ {
        REGISTRY.iter().filter(|g| self.is_enabled(g.slug))
    }
}

pub fn usage_games_section() -> String {
    let mut lines = Vec::with_capacity(REGISTRY.len());
    for entry in REGISTRY {
        let extra = if entry.slug == "onuw" {
            format!(" ({} roles)", ALL_ROLES.len())
        } else {
            String::new()
        };
        lines.push(format!("  {:<16} {}{}", entry.slug, entry.summary, extra));
    }
    lines.join("\n")
}

pub(crate) fn send_games_status(
    reply_to: &ChatMessage,
    active: Option<&'static GameEntry>,
    in_progress: bool,
    enabled: &EnabledGames,
) -> Result<()> {
    let active_label: Text = match active {
        Some(entry) if in_progress => irc_format!(
            "{} ({}) {}",
            entry.summary.bold(),
            entry.slug.bold().cyan(),
            "— game in progress".yellow(),
        ),
        Some(entry) => irc_format!("{} ({})", entry.summary.bold(), entry.slug.bold().cyan()),
        None => "none (idle)".grey(),
    };
    reply_to.reply(irc_format!(
        "🎮 Active game: {active_label} · Enabled: {} · Admins switch with {} (or {}).",
        enabled.slug_list().bold().cyan(),
        "!game <name>".bold(),
        "!game none".bold(),
    ))
}

#[cfg(test)]
#[path = "games_test.rs"]
mod test;

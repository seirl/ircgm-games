//! Game registry and per-game dispatch for `ircgm-games-bot`.

use std::future::Future;
use std::pin::Pin;

use ircgm::prelude::*;
use ircgm_onuw::lobby::{handle_catalog_query as onuw_catalog_query, LobbyPhaseTracker};
use ircgm_onuw::{
    run_single_game_tracked as run_onuw_game, DeckSpec, GameConfig as OnuwGameConfig,
    PlayOutcome as OnuwOutcome, ALL_ROLES,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameRunOutcome {
    Completed { reveal: Option<Vec<Text>> },
    NotEnoughPlayers,
}

pub type GameFuture<'a> = Pin<Box<dyn Future<Output = Result<GameRunOutcome>> + Send + 'a>>;

pub struct GameEntry {
    pub slug: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
    pub help_hint: Option<&'static str>,
    pub validate_args: fn(&str) -> std::result::Result<(), String>,
    pub handle_query: fn(&Room, &ChatMessage) -> Result<bool>,
    pub run:
        for<'a> fn(&'a Bot, &'a Room, User, &'a str, bool, &'a LobbyPhaseTracker) -> GameFuture<'a>,
}

fn validate_onuw_args(raw: &str) -> std::result::Result<(), String> {
    let trimmed = raw.trim();
    if DeckSpec::parse(trimmed).is_some() {
        Ok(())
    } else {
        Err(format!(
            "Unknown pack or scenario '{trimmed}' — try !scenarios!"
        ))
    }
}

fn run_onuw<'a>(
    bot: &'a Bot,
    lobby: &'a Room,
    starter: User,
    raw_args: &'a str,
    fast: bool,
    phase_tracker: &'a LobbyPhaseTracker,
) -> GameFuture<'a> {
    Box::pin(async move {
        let spec = DeckSpec::parse(raw_args.trim()).unwrap_or_default();
        let cfg = if fast {
            OnuwGameConfig::fast_mode()
        } else {
            OnuwGameConfig::default()
        };
        run_onuw_game(bot, lobby, starter, spec, &cfg, Some(phase_tracker))
            .await
            .map(|o| match o {
                OnuwOutcome::Completed(reveal) => GameRunOutcome::Completed {
                    reveal: Some(reveal.into_lines()),
                },
                OnuwOutcome::NotEnoughPlayers => GameRunOutcome::NotEnoughPlayers,
            })
    })
}

pub static REGISTRY: &[GameEntry] = &[GameEntry {
    slug: "onuw",
    aliases: &["werewolf"],
    summary: "One Night Ultimate Werewolf",
    help_hint: Some(
        "Start with !onuw [pack|scenario] · !reveal post-game · browse with !scenarios, !roles, !role <name>, !marks, !artifacts",
    ),
    validate_args: validate_onuw_args,
    handle_query: onuw_catalog_query,
    run: run_onuw,
}];

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

pub(crate) fn send_games_catalog(lobby: &Room, enabled: &EnabledGames) -> Result<()> {
    lobby.say(irc_format!(
        "{} Enabled games: {}",
        "🎮".bold(),
        enabled.slug_list().bold().cyan(),
    ))?;
    for entry in enabled.entries() {
        if let Some(hint) = entry.help_hint {
            lobby.say(irc_format!(
                "  {} — {} ({})",
                entry.slug.bold().cyan(),
                entry.summary,
                hint,
            ))?;
        } else {
            lobby.say(irc_format!("  {} — {}", entry.slug.bold().cyan(), entry.summary))?;
        }
    }
    if enabled.slugs().len() == 1 {
        let only = enabled.slugs()[0];
        let cmd = format!("!{only}");
        lobby.say(irc_format!(
            "Start a game with {} (or {} to cancel).",
            cmd.bold().cyan(),
            "!abort".bold(),
        ))?;
    } else {
        lobby.say(irc_format!(
            "Start a game with {}, {}, or {} (or {} to cancel).",
            "!play <game>".bold(),
            "!<game>".bold(),
            "!random".bold(),
            "!abort".bold(),
        ))?;
    }
    Ok(())
}

pub(crate) fn handle_idle_query(
    lobby: &Room,
    msg: &ChatMessage,
    enabled: &EnabledGames,
) -> Result<bool> {
    for entry in enabled.entries() {
        if (entry.handle_query)(lobby, msg)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) struct LaunchRequest {
    pub(crate) entry: &'static GameEntry,
    pub(crate) args: String,
    pub(crate) was_random: bool,
}

fn build_launch_request(
    entry: &'static GameEntry,
    rest: &str,
    enabled: &EnabledGames,
) -> std::result::Result<LaunchRequest, String> {
    if !enabled.is_enabled(entry.slug) {
        return Err(format!(
            "{} is not enabled in this lobby — enabled games: {}.",
            entry.slug,
            enabled.slug_list()
        ));
    }
    (entry.validate_args)(rest)?;
    Ok(LaunchRequest {
        entry,
        args: rest.to_owned(),
        was_random: false,
    })
}

pub(crate) fn parse_launch_command(
    msg: &ChatMessage,
    enabled: &EnabledGames,
) -> std::result::Result<Option<LaunchRequest>, String> {
    if let Some(raw_args) = msg.cmd_args("!play").or_else(|| msg.cmd_args("!start")) {
        let trimmed = raw_args.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "Usage: !play <game> or !random — enabled: {}.",
                enabled.slug_list()
            ));
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let first = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();

        let Some(entry) = find_entry(first) else {
            return Err(format!(
                "Unknown game '{first}' — enabled: {}.",
                enabled.slug_list()
            ));
        };
        return build_launch_request(entry, rest, enabled).map(Some);
    }

    if msg.is_cmd("!random") {
        let slugs = enabled.slugs();
        let picked = slugs[fastrand::usize(..slugs.len())];
        let entry = find_entry(picked).expect("enabled slug must exist in REGISTRY");
        return Ok(Some(LaunchRequest {
            entry,
            args: String::new(),
            was_random: true,
        }));
    }

    // Shorthand `!<game> [args]` (e.g. `!onuw base`).
    let trimmed = msg.text.trim();
    if let Some(after_bang) = trimmed.strip_prefix('!') {
        let mut parts = after_bang.splitn(2, char::is_whitespace);
        let word = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();
        if let Some(entry) = find_entry(word) {
            return build_launch_request(entry, rest, enabled).map(Some);
        }
    }

    Ok(None)
}

pub(crate) fn is_launch_attempt(msg: &ChatMessage) -> bool {
    if msg.is_cmd("!play") || msg.is_cmd("!start") || msg.is_cmd("!random") {
        return true;
    }
    msg.text
        .trim()
        .strip_prefix('!')
        .and_then(|s| s.split_whitespace().next())
        .and_then(find_entry)
        .is_some()
}

#[cfg(test)]
#[path = "games_test.rs"]
mod test;

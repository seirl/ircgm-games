//! Game-lobby switcher supervisor (`!game`, `!stop`) and admin `!quit` loop.

use ircgm::prelude::*;

use crate::games::{
    default_coup_lobby, default_onuw_lobby, default_secret_hitler_lobby, find_entry,
    send_games_status, EnabledGames, GameEntry, GameKind, GameLobby,
};

fn is_admin_msg(msg: &ChatMessage, admins: &[String]) -> bool {
    let Some(acct) = msg.account.as_deref() else {
        return false;
    };
    admins.iter().any(|a| a.eq_ignore_ascii_case(acct))
}

enum Control {
    Shutdown,
    Switch(Option<&'static GameEntry>, Box<ChatMessage>),
    Stop(Box<ChatMessage>),
}

#[derive(Clone, Copy)]
enum SupervisorMode {
    BotIdle,
    GameIdle(&'static GameEntry),
    Recruiting(&'static GameEntry),
    Playing(&'static GameEntry),
}

impl SupervisorMode {
    fn active_entry(self) -> Option<&'static GameEntry> {
        match self {
            Self::BotIdle => None,
            Self::GameIdle(e) | Self::Recruiting(e) | Self::Playing(e) => Some(e),
        }
    }
}

enum SessionExit {
    Shutdown,
    Switch(Option<&'static GameEntry>),
}

/// Evaluates supervisor meta-commands (`!games`, `!game`, `!stop`, and idle `!help`) according
/// to the current phase.
fn handle_supervisor_msg(
    msg: &ChatMessage,
    lobby: &Room,
    admins: &[String],
    enabled: &EnabledGames,
    mode: SupervisorMode,
) -> Option<Control> {
    if !(msg.is_in(lobby) || msg.is_dm()) {
        return None;
    }

    let active = mode.active_entry();
    let in_progress = matches!(mode, SupervisorMode::Playing(_));

    if msg.is_cmd("!games") || (active.is_none() && msg.is_cmd("!help")) {
        let _ = send_games_status(msg, active, in_progress, enabled);
        return None;
    }

    if msg.is_cmd("!stop") {
        if !is_admin_msg(msg, admins) {
            return None;
        }
        return match mode {
            SupervisorMode::BotIdle | SupervisorMode::GameIdle(_) => {
                let _ = msg.reply("No game is running.".yellow());
                None
            }
            SupervisorMode::Recruiting(_) | SupervisorMode::Playing(_) => {
                Some(Control::Stop(Box::new(msg.clone())))
            }
        };
    }

    let raw = msg.cmd_args("!game")?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        let _ = send_games_status(msg, active, in_progress, enabled);
        return None;
    }

    if !is_admin_msg(msg, admins) {
        return None;
    }

    if let SupervisorMode::Playing(entry) = mode {
        let _ = msg.reply(irc_format!(
            "⏳ A game of {} is in progress — finish it or run {} first!",
            entry.slug.bold().cyan(),
            "!stop".bold(),
        ));
        return None;
    }

    let first = trimmed.split_whitespace().next().unwrap_or("");
    if matches!(
        first.to_ascii_lowercase().as_str(),
        "none" | "off" | "idle" | "stop"
    ) {
        if active.is_none() {
            let _ = msg.reply("Bot is already idle.".yellow());
            return None;
        }
        return Some(Control::Switch(None, Box::new(msg.clone())));
    }

    let Some(entry) = find_entry(first) else {
        let _ = msg.reply(
            format_args!(
                "Unknown game '{first}' — enabled: {}.",
                enabled.slug_list()
            )
            .yellow(),
        );
        return None;
    };

    if !enabled.is_enabled(entry.slug) {
        let _ = msg.reply(
            format_args!(
                "{} is not enabled in this lobby — enabled games: {}.",
                entry.slug,
                enabled.slug_list()
            )
            .yellow(),
        );
        return None;
    }

    if active.is_some_and(|cur| cur.slug == entry.slug) {
        let _ = msg.reply(format_args!("Already running {}!", entry.slug).yellow());
        return None;
    }

    Some(Control::Switch(Some(entry), Box::new(msg.clone())))
}

fn supervisor_listener(
    bot: &Bot,
    lobby: &Room,
    admins: &[String],
    enabled: &EnabledGames,
    mode: SupervisorMode,
) -> impl std::future::Future<Output = Control> + Send + 'static {
    let inbox = bot.inbox();
    let admin_list = admins.to_vec();
    let lobby = lobby.clone();
    let enabled = enabled.clone();
    async move {
        inbox
            .on_map(move |msg| handle_supervisor_msg(msg, &lobby, &admin_list, &enabled, mode))
            .await
    }
}

fn announce_switch(
    lobby: &Room,
    from: &'static GameEntry,
    to: Option<&'static GameEntry>,
    trigger: &ChatMessage,
) -> Result<()> {
    match to {
        Some(next_entry) => {
            tracing::info!(
                from = from.slug,
                to = next_entry.slug,
                admin = %trigger.nick,
                lobby = %lobby.name(),
                "switching game lobby"
            );
            if trigger.is_dm() {
                trigger.reply(irc_format!(
                    "🔄 Switched {} from {} to {}.",
                    lobby.name(),
                    from.slug.bold().cyan(),
                    next_entry.slug.bold().cyan(),
                ))?;
            }
            lobby.say(irc_format!(
                "🔄 Switching lobby from {} to {}...",
                from.slug.bold().cyan(),
                next_entry.slug.bold().cyan(),
            ))?;
        }
        None => {
            tracing::info!(
                from = from.slug,
                admin = %trigger.nick,
                lobby = %lobby.name(),
                "returning lobby to idle"
            );
            if trigger.is_dm() {
                trigger.reply(
                    format_args!(
                        "🛑 Closed {} lobby in {} — bot is now idle.",
                        from.slug,
                        lobby.name(),
                    )
                    .yellow(),
                )?;
            }
            lobby.say(
                format_args!("🛑 Closed {} lobby — bot is now idle.", from.slug).yellow(),
            )?;
        }
    }
    Ok(())
}

fn announce_admin_stop(
    lobby: &Room,
    entry: &'static GameEntry,
    trigger: &ChatMessage,
    kind: &str,
) -> Result<()> {
    tracing::info!(
        game = entry.slug,
        kind,
        admin = %trigger.nick,
        lobby = %lobby.name(),
        "admin stopped game"
    );
    trigger.reply(format_args!("🛑 Stopped {} {kind}.", entry.slug).yellow())?;
    if trigger.is_dm() {
        lobby.say(format_args!("🛑 {} {kind} was stopped by an admin.", entry.slug).yellow())?;
    }
    Ok(())
}

async fn run_game_session<G: GameLobby>(
    bot: &Bot,
    lobby: &Room,
    admins: &[String],
    enabled: &EnabledGames,
    entry: &'static GameEntry,
    mut game: G,
) -> Result<SessionExit> {
    loop {
        // Phase 1: game lobby idle — waiting for `!start`.
        // `!game <name|none>` switches/closes; `!stop` reports no game is running.
        let launch = match game
            .wait_for_start(lobby)
            .until(async {
                bot.shutdown_requested().await;
                Control::Shutdown
            })
            .or_until(supervisor_listener(
                bot,
                lobby,
                admins,
                enabled,
                SupervisorMode::GameIdle(entry),
            ))
            .await
        {
            Outcome::Completed(res) => res?,
            Outcome::Stopped(Control::Shutdown) => return Ok(SessionExit::Shutdown),
            Outcome::Stopped(Control::Switch(next, trigger)) => {
                announce_switch(lobby, entry, next, &trigger)?;
                return Ok(SessionExit::Switch(next));
            }
            Outcome::Stopped(Control::Stop(_)) => unreachable!("!stop does not cancel GameIdle"),
        };

        // Phase 2: open recruitment (`!join` / `!play` / `!go`).
        // `!game <name|none>` switches/closes; admin `!stop` cancels recruitment and stays in lobby.
        let round = match game
            .recruit(bot, lobby, launch)
            .until(async {
                bot.shutdown_requested().await;
                Control::Shutdown
            })
            .or_until(supervisor_listener(
                bot,
                lobby,
                admins,
                enabled,
                SupervisorMode::Recruiting(entry),
            ))
            .await
        {
            Outcome::Completed(res) => match res? {
                Some(round) => round,
                None => continue,
            },
            Outcome::Stopped(Control::Shutdown) => return Ok(SessionExit::Shutdown),
            Outcome::Stopped(Control::Switch(next, trigger)) => {
                announce_switch(lobby, entry, next, &trigger)?;
                return Ok(SessionExit::Switch(next));
            }
            Outcome::Stopped(Control::Stop(trigger)) => {
                announce_admin_stop(lobby, entry, &trigger, "lobby")?;
                continue;
            }
        };

        // Phase 3: active dealt round.
        // `!game <name|none>` is rejected; admin `!stop` aborts the round and returns to Phase 1.
        match game
            .play_round(bot, lobby, round)
            .until(async {
                bot.shutdown_requested().await;
                Control::Shutdown
            })
            .or_until(supervisor_listener(
                bot,
                lobby,
                admins,
                enabled,
                SupervisorMode::Playing(entry),
            ))
            .await
        {
            Outcome::Completed(res) => res?,
            Outcome::Stopped(Control::Shutdown) => return Ok(SessionExit::Shutdown),
            Outcome::Stopped(Control::Stop(trigger)) => {
                announce_admin_stop(lobby, entry, &trigger, "game")?;
            }
            Outcome::Stopped(Control::Switch(..)) => {
                unreachable!("!game switch is rejected during Playing")
            }
        }
    }
}

pub async fn run_lobby(
    bot: &Bot,
    lobby: &Room,
    admins: &[String],
    enabled: &EnabledGames,
) -> Result<()> {
    lobby.say(irc_format!(
        "🎮 {} Enabled games: {}. Admins can start a game lobby with {} (or type {} to list).",
        "Bot ready (idle)!".bold().green(),
        enabled.slug_list().bold().cyan(),
        "!game <name>".bold(),
        "!games".bold(),
    ))?;

    let mut active: Option<&'static GameEntry> = None;

    loop {
        let Some(entry) = active else {
            let mut inbox = bot.inbox().lossy();
            let next = loop {
                let msg = match inbox.next_chat().until(bot.shutdown_requested()).await {
                    Outcome::Completed(res) => res?,
                    Outcome::Stopped(()) => break None,
                };
                if let Some(Control::Switch(Some(entry), trigger)) =
                    handle_supervisor_msg(&msg, lobby, admins, enabled, SupervisorMode::BotIdle)
                {
                    tracing::info!(
                        game = entry.slug,
                        admin = %trigger.nick,
                        lobby = %lobby.name(),
                        "loading game lobby"
                    );
                    if trigger.is_dm() {
                        trigger.reply(irc_format!(
                            "🎮 Loaded {} lobby in {}.",
                            entry.slug.bold().cyan(),
                            lobby.name(),
                        ))?;
                    }
                    break Some(entry);
                }
            };
            match next {
                Some(entry) => active = Some(entry),
                None => break,
            }
            continue;
        };

        let exit = match entry.kind {
            GameKind::Onuw => {
                run_game_session(bot, lobby, admins, enabled, entry, default_onuw_lobby()).await?
            }
            GameKind::SecretHitler => {
                run_game_session(
                    bot,
                    lobby,
                    admins,
                    enabled,
                    entry,
                    default_secret_hitler_lobby(),
                )
                .await?
            }
            GameKind::Coup => {
                run_game_session(bot, lobby, admins, enabled, entry, default_coup_lobby()).await?
            }
        };

        match exit {
            SessionExit::Shutdown => break,
            SessionExit::Switch(next) => active = next,
        }
    }

    Ok(())
}

pub async fn admin_quit_loop(bot: &Bot, lobby: &Room, admins: &[String]) -> Result<bool> {
    if admins.is_empty() {
        bot.shutdown_requested().await;
        return Ok(false);
    }

    let mut inbox = bot.inbox().lossy();
    while let Outcome::Completed(msg) = inbox.wait_cmd("!quit").until(bot.shutdown_requested()).await
    {
        let msg = msg?;
        if (msg.is_dm() || msg.is_in(lobby)) && is_admin_msg(&msg, admins) {
            msg.reply("👋 Shutting down bot!".yellow())?;
            bot.quit(Some("Requested by admin")).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

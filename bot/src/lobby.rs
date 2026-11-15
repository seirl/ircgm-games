//! Game-agnostic lobby supervisor and admin command loops.

use ircgm::prelude::*;
use ircgm_onuw::lobby::{send_lobby_help as send_onuw_lobby_help, LobbyPhaseTracker};

use crate::games::{
    handle_idle_query, is_launch_attempt, parse_launch_command, send_games_catalog, EnabledGames,
    GameRunOutcome,
};

fn is_admin_msg(msg: &ChatMessage, admins: &[String]) -> bool {
    let Some(acct) = msg.account.as_deref() else {
        return false;
    };
    admins.iter().any(|a| a.eq_ignore_ascii_case(acct))
}

fn handle_admin_fast_command(msg: &ChatMessage, admins: &[String], fast: &mut bool) -> Result<bool> {
    let Some(raw) = msg.cmd_args("!fast") else {
        return Ok(false);
    };
    if !is_admin_msg(msg, admins) {
        return Ok(true);
    }
    let arg = raw.trim().to_ascii_lowercase();
    let next = match arg.as_str() {
        "" => !*fast,
        "on" | "true" | "1" | "yes" => true,
        "off" | "false" | "0" | "no" => false,
        _ => {
            msg.reply("Usage: !fast [on|off]".yellow())?;
            return Ok(true);
        }
    };
    *fast = next;
    if next {
        msg.reply("⚡ Fast mode enabled for upcoming games.".green())?;
    } else {
        msg.reply("🐢 Fast mode disabled for upcoming games.".cyan())?;
    }
    Ok(true)
}

enum StopReason {
    Shutdown,
    Admin(Box<ChatMessage>),
    Aborted(Box<ChatMessage>),
}

pub async fn run_lobby(
    bot: &Bot,
    lobby: &Room,
    admins: &[String],
    enabled: &EnabledGames,
) -> Result<()> {
    let mut fast = false;
    let mut pending_reveal: Option<Vec<Text>> = None;
    let single_onuw = enabled.slugs() == ["onuw"];

    if single_onuw {
        send_onuw_lobby_help(lobby)?;
    } else {
        lobby.say(irc_format!(
            "{} {} Enabled games: {}. Type {}, {}, or {}.",
            "🎮".bold(),
            "Bot ready!".bold().green(),
            enabled.slug_list().bold().cyan(),
            "!play <game>".bold(),
            "!random".bold(),
            "!games".bold(),
        ))?;
    }

    loop {
        let Some((starter, launch)) = ({
            let mut inbox = lobby.inbox().lossy();
            loop {
                let idle_msg = match inbox.next_chat().until(bot.shutdown_requested()).await {
                    Outcome::Completed(res) => res?,
                    Outcome::Stopped(()) => break None,
                };

                if idle_msg.is_cmd("!help") && single_onuw {
                    send_onuw_lobby_help(lobby)?;
                    continue;
                }
                if idle_msg.is_cmd("!games") || idle_msg.is_cmd("!help") {
                    send_games_catalog(lobby, enabled)?;
                    continue;
                }
                if idle_msg.is_cmd("!reveal") {
                    match pending_reveal.take() {
                        Some(lines) => {
                            for line in lines {
                                lobby.say(line)?;
                            }
                        }
                        None => {
                            idle_msg.reply("Nothing left to reveal from the last game!".yellow())?;
                        }
                    }
                    continue;
                }
                if handle_admin_fast_command(&idle_msg, admins, &mut fast)? {
                    continue;
                }
                if handle_idle_query(lobby, &idle_msg, enabled)? {
                    continue;
                }

                match parse_launch_command(&idle_msg, enabled) {
                    Ok(None) => continue,
                    Ok(Some(req)) => {
                        if req.was_random {
                            lobby.say(irc_format!(
                                "{} {} rolled {}!",
                                "🎲".bold(),
                                idle_msg.sender.nick().bold(),
                                req.entry.slug.bold().cyan(),
                            ))?;
                        }
                        pending_reveal = None;
                        break Some((idle_msg.sender.clone(), req));
                    }
                    Err(err_text) => {
                        idle_msg.reply(err_text.yellow())?;
                        continue;
                    }
                }
            }
        }) else {
            break;
        };

        let slug = launch.entry.slug;
        tracing::info!(
            game = slug,
            starter = %starter.nick(),
            lobby = %lobby.name(),
            fast,
            "starting game"
        );

        let admin_list = admins.to_vec();
        let lobby_for_admin = lobby.clone();
        let admin_stop = bot.inbox().on_map(move |m| {
            (is_admin_msg(m, &admin_list)
                && m.is_cmd("!stop")
                && (m.is_dm() || m.is_in(&lobby_for_admin)))
            .then(|| StopReason::Admin(Box::new(m.clone())))
        });

        let abort_scope = InboxScope::new().room(lobby).dm_from(&starter);
        let starter_for_abort = starter.clone();
        let lobby_for_abort = lobby.clone();
        let abort_stop = bot.inbox_scope(abort_scope).on_map(move |m| {
            if !(m.is_cmd("!abort") || m.is_cmd("!stop")) {
                return None;
            }
            let is_starter = m.sender == starter_for_abort;
            let starter_gone =
                !starter_for_abort.is_online() || !lobby_for_abort.has_user(&starter_for_abort);
            if is_starter || (starter_gone && m.is_in(&lobby_for_abort)) {
                Some(StopReason::Aborted(Box::new(m.clone())))
            } else {
                None
            }
        });

        let phase_tracker = LobbyPhaseTracker::new();
        let phase_for_busy = phase_tracker.clone();
        let starter_for_busy = starter.clone();
        let lobby_for_busy = lobby.clone();
        let enabled_for_busy = enabled.clone();
        let busy_responder = lobby.inbox().on_map(move |msg| {
            if msg.is_cmd("!games") {
                let _ = send_games_catalog(&lobby_for_busy, &enabled_for_busy);
            } else if msg.is_cmd("!reveal") {
                let _ = msg.reply(
                    "⏳ Finish the current game first — !reveal is available after the round ends!"
                        .yellow(),
                );
            } else if is_launch_attempt(msg) {
                if phase_for_busy.is_recruiting() {
                    return None;
                }
                let abort_hint = if starter_for_busy.is_online()
                    && lobby_for_busy.has_user(&starter_for_busy)
                {
                    format!("{} can type !abort", starter_for_busy.nick())
                } else {
                    "type !abort".to_owned()
                };
                let _ = lobby_for_busy.say(
                    format_args!("⏳ {slug} is already in progress ({abort_hint} to cancel)!")
                        .yellow(),
                );
            }
            None::<StopReason>
        });

        let outcome =
            (launch.entry.run)(bot, lobby, starter, &launch.args, fast, &phase_tracker)
                .until(async {
                    bot.shutdown_requested().await;
                    StopReason::Shutdown
                })
                .or_until(admin_stop)
                .or_until(abort_stop)
                .or_until(busy_responder)
                .await;

        match outcome {
            Outcome::Completed(Ok(GameRunOutcome::Completed { reveal })) => {
                tracing::info!(game = slug, "game completed");
                let has_reveal = reveal.is_some();
                pending_reveal = reveal;
                if single_onuw && has_reveal {
                    lobby.say(irc_format!(
                        "🏁 {} Discuss what happened, then type {} for the full reveal — or {} for another round!",
                        "Round complete!".bold().green(),
                        "!reveal".bold().yellow(),
                        "!onuw [pack|scenario]".bold().cyan(),
                    ))?;
                } else if has_reveal {
                    lobby.say(irc_format!(
                        "🏁 {} finished — lobby is open! Discuss what happened and type {} for the full reveal, or {}, {}, {}.",
                        slug.bold().cyan(),
                        "!reveal".bold().yellow(),
                        "!play <game>".bold(),
                        "!random".bold(),
                        "!games".bold(),
                    ))?;
                } else {
                    lobby.say(irc_format!(
                        "🏁 {} finished — lobby is open! Type {}, {}, or {}.",
                        slug.bold().cyan(),
                        "!play <game>".bold(),
                        "!random".bold(),
                        "!games".bold(),
                    ))?;
                }
            }
            Outcome::Completed(Ok(GameRunOutcome::NotEnoughPlayers)) => {
                tracing::info!(game = slug, "game ended: not enough players");
            }
            Outcome::Completed(Err(err)) => {
                let lost_lobby = matches!(
                    &err,
                    Error::BotLeftRoom { channel, .. } if lobby.casemap().str_eq(channel, lobby.name())
                );
                if err.is_fatal() || lost_lobby {
                    return Err(err);
                }
                tracing::warn!(game = slug, error = %err, "game ended early");
                lobby.say(
                    format_args!("⚠️ {slug} ended early: {err} — lobby is open again!").yellow(),
                )?;
            }
            Outcome::Stopped(StopReason::Shutdown) => break,
            Outcome::Stopped(StopReason::Admin(admin_msg)) => {
                tracing::info!(game = slug, admin = %admin_msg.nick, "game stopped by admin");
                admin_msg.reply(format_args!("🛑 Stopped {slug}.").yellow())?;
                if admin_msg.is_dm() {
                    lobby.say(format_args!("🛑 {slug} was stopped by an admin.").yellow())?;
                }
            }
            Outcome::Stopped(StopReason::Aborted(abort_msg)) => {
                tracing::info!(game = slug, by = %abort_msg.nick, "game aborted");
                if abort_msg.is_dm() {
                    abort_msg.reply(format_args!("🛑 Aborted {slug}.").yellow())?;
                }
                lobby.say(irc_format!(
                    "🛑 {} {}",
                    abort_msg.sender.nick().bold(),
                    format!("aborted {slug} — lobby is open!").yellow(),
                ))?;
            }
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

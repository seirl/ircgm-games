//! Stateful three-phase lobby (`SecretHitlerLobby`), standalone loop (`run_lobby_bot`), pre-game
//! recruitment (`recruit_players`), and `!rules` / `!tracks` catalog commands.

use ircgm::prelude::*;
use tokio::time::Instant;

use crate::game::{run_dealt_game, GameConfig, GameReveal, GameSetup};
use crate::state::{
    format_track_reference, hitler_knows_fascists, role_distribution, MAX_PLAYERS, MIN_PLAYERS,
};

pub fn format_role_breakdown(n: usize) -> Option<Text> {
    let (libs, fas) = role_distribution(n)?;
    let fas_w = if fas == 1 { "Fascist" } else { "Fascists" };
    let vis = if hitler_knows_fascists(n) {
        "Hitler knows Fascist"
    } else {
        "Hitler does NOT know Fascists"
    };
    Some(irc_format!(
        "{}, {}, {} {}",
        format!("{libs} Liberals").bold().cyan(),
        format!("{fas} {fas_w}").bold().red(),
        "1 Hitler".bold().orange(),
        format!("({vis})").grey(),
    ))
}

pub fn send_lobby_help(lobby: &Room) -> Result<()> {
    let sep = " · ".grey();
    lobby.say(irc_format!(
        "🎩 {} {} — {}{sep}{}{sep}{}{sep}{}",
        "Secret Hitler".bold().red(),
        "(5–10p)".grey(),
        "!start".bold().green(),
        "!rules".bold().cyan(),
        "!tracks [5p..10p]".bold().cyan(),
        "!reveal".bold().yellow(),
    ))
}

pub fn send_open_lobby_help(reply_to: &ChatMessage, host: &User) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🎩 {} — {}{sep}{}{sep}{}{sep}{}{sep}{}",
        "Open Lobby".bold().yellow(),
        "!join".bold().green(),
        "!leave".bold(),
        "!players".bold().cyan(),
        "!rules".bold().cyan(),
        "!tracks [5p..10p]".bold().cyan(),
    ))?;
    reply_to.reply(irc_format!(
        "  {} {}{sep}{}",
        format!("Host ({}):", host.nick()).bold(),
        "!go".bold().green(),
        "!abort".bold().orange(),
    ))
}

pub fn send_active_game_help(reply_to: &ChatMessage) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🎩 {} — {}{sep}{}{sep}{}{sep}{}{sep}{}{sep}{}{sep}Gov: {}{sep}Vote (DM): {} / {}{sep}Cancel: {}",
        "Active Game".bold().yellow(),
        "!board".bold().cyan(),
        "!players".bold().cyan(),
        "!eligible".bold().cyan(),
        "!tracks".bold().cyan(),
        "!rules".bold().cyan(),
        "!time".bold().cyan(),
        "!chancellor <nick>".bold().green(),
        "ja".bold().green(),
        "nein".bold().red(),
        "!abort".bold().orange(),
    ))
}

pub fn send_rules_summary(reply_to: &ChatMessage) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "📜 {} — {} (5–6p: Hitler knows Fascist; 7–10p: Hitler is blind){sep}{} deck (reshuffled below 3){sep}{} term-limits last elected Chan (+ Pres when >5 alive).",
        "Secret Hitler Rules".bold().cyan(),
        "Liberals vs Fascists + Hitler".bold(),
        "6 Liberal + 11 Fascist".bold(),
        "Nomination".bold(),
    ))?;
    reply_to.reply(irc_format!(
        "  {}: secret DM {} / {} (strict majority passes; 3 fails/vetoes → {} top policy, clearing term limits). {}: Pres draws 3 & discards 1, Chan enacts 1 of 2 (silent in session; {} at 5F). {}: 5 Liberal or execute Hitler vs 6 Fascist or elect Hitler Chan at 3F+!",
        "Vote".bold(),
        "ja".bold().green(),
        "nein".bold().red(),
        "Chaos".bold().orange(),
        "Legislation".bold(),
        "Veto".bold().yellow(),
        "Win".bold(),
    ))
}

pub fn send_tracks_catalog(reply_to: &ChatMessage, raw_filter: Option<&str>) -> Result<()> {
    const BRACKETS: &[(&str, usize)] = &[("5–6p", 5), ("7–8p", 7), ("9–10p", 9)];
    if let Some(q) = raw_filter.map(str::trim).filter(|s| !s.is_empty()) {
        let lower = q.to_ascii_lowercase();
        if let Some((lbl, n)) = match lower.as_str() {
            "5-6" | "5-6p" => Some(BRACKETS[0]),
            "7-8" | "7-8p" => Some(BRACKETS[1]),
            "9-10" | "9-10p" => Some(BRACKETS[2]),
            _ => None,
        } {
            return reply_to.reply(format_track_reference(lbl, n));
        }
        let num = lower
            .strip_suffix("players")
            .or_else(|| lower.strip_suffix("player"))
            .or_else(|| lower.strip_suffix('p'))
            .unwrap_or(&lower)
            .trim()
            .parse::<usize>()
            .ok();
        if let Some((n, roles)) = num.and_then(|n| Some((n, format_role_breakdown(n)?))) {
            let lbl = match n {
                5..=6 => "5–6p",
                7..=8 => "7–8p",
                _ => "9–10p",
            };
            reply_to.reply(irc_format!(
                "🎩 {} — {roles}",
                format!("{n}-Player Secret Hitler").bold().cyan()
            ))?;
            return reply_to.reply(format_track_reference(lbl, n));
        }
        return reply_to.reply(
            format_args!(
                "Invalid player count '{q}' — Secret Hitler supports 5–10 players (try !tracks 5p, !tracks 7p, or !tracks 9p)."
            )
            .yellow(),
        );
    }

    reply_to.reply(irc_format!(
        "🏛️ {} — {} for exact role counts:",
        "Secret Hitler Fascist Boards (5–10p)".bold().cyan(),
        "!tracks <5p..10p>".bold(),
    ))?;
    for &(lbl, n) in BRACKETS {
        reply_to.reply(format_track_reference(lbl, n))?;
    }
    Ok(())
}

/// Shared catalog handler for `!rules` and `!tracks` / `!boards` / `!powers`.
pub fn handle_catalog_query(_room: &Room, msg: &ChatMessage) -> Result<bool> {
    if msg.is_cmd("!rules") {
        send_rules_summary(msg)?;
        return Ok(true);
    }
    if let Some(arg) = ["!tracks", "!track", "!boards", "!powers"]
        .iter()
        .find_map(|c| msg.cmd_args(c))
    {
        send_tracks_catalog(msg, Some(arg))?;
        return Ok(true);
    }
    Ok(false)
}

fn remove_lobby_player(
    players: &mut Vec<User>,
    user: &User,
    room: &Room,
    announce_empty: bool,
) -> Result<bool> {
    let was_host = players.first() == Some(user);
    let before = players.len();
    players.retain(|u| u != user);
    if players.is_empty() {
        if announce_empty {
            room.say("Lobby emptied and closed.".yellow())?;
        }
        return Ok(true);
    }
    if players.len() < before && was_host {
        room.say(irc_format!(
            "{} left — {} is now lobby host!",
            user.nick().bold(),
            players[0].nick().bold().yellow(),
        ))?;
    }
    Ok(false)
}

pub async fn recruit_players(
    room: &Room,
    host: User,
    config: &GameConfig,
) -> Result<Option<Vec<User>>> {
    let mut inbox = room.inbox();
    let mut players = vec![host.clone()];
    let sep = " · ".grey();
    room.say(irc_format!(
        "🎩 {} opened a {} lobby {}! Type {} to play ({}s).",
        host.nick().bold().yellow(),
        "Secret Hitler".bold().red(),
        "(5–10p)".grey(),
        "!join".bold().green(),
        config.lobby_duration.as_secs(),
    ))?;
    room.say(irc_format!(
        "  {} {}{sep}{}{sep}{}{sep}{}",
        "Host:".bold(),
        "!go".bold().green(),
        "!abort".bold().orange(),
        "!players".bold().cyan(),
        "!tracks".bold().cyan(),
    ))?;

    let deadline = Instant::now() + config.lobby_duration;
    let recruit_loop = async {
        while players.len() < MAX_PLAYERS {
            match inbox.next_event().await? {
                Event::Chat(msg) => {
                    let is_host = players.first() == Some(&msg.sender);
                    if msg.is_cmd("!join") || msg.is_cmd("!in") || msg.is_cmd("!play") {
                        if !players.contains(&msg.sender) {
                            players.push(msg.sender.clone());
                            let n = players.len();
                            let detail = format_role_breakdown(n).unwrap_or_else(|| {
                                format!("{} more needed", MIN_PLAYERS - n).into()
                            });
                            room.say(irc_format!(
                                "{} joined! ({n}/{MAX_PLAYERS} — {detail})",
                                msg.sender.nick().bold().green()
                            ))?;
                        }
                    } else if msg.is_cmd("!leave") || msg.is_cmd("!out") || msg.is_cmd("!part") {
                        if remove_lobby_player(&mut players, &msg.sender, room, true)? {
                            break;
                        }
                    } else if msg.is_cmd("!players") || msg.is_cmd("!who") {
                        let list = players
                            .iter()
                            .map(User::nick)
                            .collect::<Vec<_>>()
                            .join(", ");
                        let n = players.len();
                        let suffix = format_role_breakdown(n).map_or_else(
                            || Text::from(format!("(need {MIN_PLAYERS}+ to start)")),
                            |b| irc_format!("({b})"),
                        );
                        msg.reply(irc_format!(
                            "👥 {} {list} {suffix}",
                            format!("Lobby ({n}/{MAX_PLAYERS}):").bold()
                        ))?;
                    } else if msg.is_cmd("!help") {
                        send_open_lobby_help(&msg, &players[0])?;
                    } else if msg.is_cmd("!reveal") {
                        msg.reply(
                            "⏳ Finish the current game first — !reveal is available after the game ends!"
                                .yellow(),
                        )?;
                    } else if handle_catalog_query(room, &msg)? {
                        continue;
                    } else if msg.is_cmd("!go") || msg.is_cmd("!force") {
                        if !is_host {
                            msg.reply(irc_format!(
                                "{} {}",
                                "Only the lobby host can start early with".yellow(),
                                "!go!".bold().yellow()
                            ))?;
                        } else if players.len() < MIN_PLAYERS {
                            msg.reply(
                                format_args!(
                                    "Need at least {MIN_PLAYERS} players for Secret Hitler (currently {}).",
                                    players.len()
                                )
                                .yellow(),
                            )?;
                        } else {
                            break;
                        }
                    } else if msg.is_cmd("!start") {
                        let reply = if is_host {
                            irc_format!(
                                "Lobby is already open — use {} to start now ({MIN_PLAYERS}–{MAX_PLAYERS} players) or {} to cancel.",
                                "!go".bold().green(),
                                "!abort".bold().orange()
                            )
                        } else if players.contains(&msg.sender) {
                            irc_format!(
                                "Lobby is already open (hosted by {}) — waiting for more players or {}!",
                                players[0].nick().bold(),
                                "!go".bold().green()
                            )
                        } else {
                            irc_format!(
                                "Lobby is already open (hosted by {}) — type {} to join!",
                                players[0].nick().bold(),
                                "!join".bold().green()
                            )
                        };
                        msg.reply(reply)?;
                    }
                }
                Event::Parted { user, .. }
                | Event::Kicked { user, .. }
                | Event::Quit { user, .. } => {
                    if remove_lobby_player(&mut players, &user, room, false)? {
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    };

    recruit_loop.run_until_deadline(deadline).await?;

    players.retain(|u| u.is_online() && room.has_user(u));
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&players.len()) {
        if !players.is_empty() {
            room.say(
                format_args!(
                    "Player count ({}) outside required {MIN_PLAYERS}–{MAX_PLAYERS} range for Secret Hitler — lobby closed.",
                    players.len()
                )
                .yellow(),
            )?;
        }
        return Ok(None);
    }
    Ok(Some(players))
}

/// A validated `!start` request ready to open pre-game recruitment.
#[derive(Debug)]
pub struct SecretHitlerLaunch {
    pub starter: User,
}

/// A recruited Secret Hitler round ready to be dealt and played.
#[derive(Debug)]
pub struct SecretHitlerRound {
    pub host: User,
    pub players: Vec<User>,
}

/// Stateful Secret Hitler channel lobby exposing explicit idle (`wait_for_start`), recruitment
/// (`recruit`), and gameplay (`play_round`) phases so callers (such as a multi-game supervisor)
/// own the outer loop and admin cancellation boundaries.
#[derive(Debug)]
pub struct SecretHitlerLobby {
    config: GameConfig,
    pending_reveal: Option<GameReveal>,
    announced_help: bool,
}

fn abort_stop_listener(
    bot: &Bot,
    lobby: &Room,
    host: &User,
) -> impl std::future::Future<Output = ChatMessage> + Send + 'static {
    let scope = InboxScope::new().room(lobby).dm_from(host);
    let inbox = bot.inbox_scope(scope);
    let host = host.clone();
    let lobby = lobby.clone();
    async move {
        inbox
            .on_map(move |m| {
                if !m.is_cmd("!abort") {
                    return None;
                }
                let is_host = m.sender == host;
                let host_gone = !host.is_online() || !lobby.has_user(&host);
                (is_host || (host_gone && m.is_in(&lobby))).then(|| m.clone())
            })
            .await
    }
}

fn handle_round_error(lobby: &Room, err: Error) -> Result<()> {
    let lost_lobby = matches!(
        &err,
        Error::BotLeftRoom { channel, .. } if lobby.casemap().str_eq(channel, lobby.name())
    );
    if err.is_fatal() || lost_lobby {
        return Err(err);
    }
    tracing::warn!(error = %err, "Secret Hitler game ended early");
    lobby.say(
        format_args!("⚠️ Secret Hitler game ended early: {err} — lobby is open again!").yellow(),
    )?;
    Ok(())
}

fn handle_abort(lobby: &Room, abort_msg: &ChatMessage) -> Result<()> {
    if abort_msg.is_dm() {
        abort_msg.reply("🛑 Aborted Secret Hitler game.".yellow())?;
    }
    lobby.say(irc_format!(
        "🛑 {} {}",
        abort_msg.sender.nick().bold(),
        "aborted the Secret Hitler game — lobby is open!".yellow(),
    ))?;
    Ok(())
}

impl SecretHitlerLobby {
    pub fn new(config: GameConfig) -> Self {
        Self {
            config,
            pending_reveal: None,
            announced_help: false,
        }
    }

    /// Phase 1: handles idle lobby commands (`!help`, `!reveal`, `!rules`, `!tracks`) until a
    /// player starts a round with `!start`.
    pub async fn wait_for_start(&mut self, lobby: &Room) -> Result<SecretHitlerLaunch> {
        if !self.announced_help {
            send_lobby_help(lobby)?;
            self.announced_help = true;
        }

        let mut inbox = lobby.inbox().lossy();
        loop {
            let msg = inbox.next_chat().await?;
            if msg.is_cmd("!help") {
                send_lobby_help(lobby)?;
            } else if msg.is_cmd("!reveal") {
                match self.pending_reveal.take() {
                    Some(reveal) => reveal.announce(lobby)?,
                    None => msg.reply("Nothing left to reveal from the last game!".yellow())?,
                }
            } else if handle_catalog_query(lobby, &msg)? {
                continue;
            } else if let Some(arg) = msg.cmd_args("!start") {
                let trimmed = arg.trim();
                if !trimmed.is_empty() {
                    msg.reply(
                        format_args!(
                            "Secret Hitler takes no arguments (got '{trimmed}') — just type !start!"
                        )
                        .yellow(),
                    )?;
                    continue;
                }
                return Ok(SecretHitlerLaunch {
                    starter: msg.sender.clone(),
                });
            }
        }
    }

    /// Phase 2: runs open pre-game recruitment (`!join`/`!leave`/`!players`/`!go`, host `!abort`).
    /// Returns `Ok(Some(round))` when 5–10 players are ready, or `Ok(None)` if cancelled/emptied
    /// or timed out without enough players.
    pub async fn recruit(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        launch: SecretHitlerLaunch,
    ) -> Result<Option<SecretHitlerRound>> {
        let outcome = recruit_players(lobby, launch.starter.clone(), &self.config)
            .until(abort_stop_listener(bot, lobby, &launch.starter))
            .await;

        match outcome {
            Outcome::Completed(Ok(Some(players))) => {
                let host = players[0].clone();
                Ok(Some(SecretHitlerRound { host, players }))
            }
            Outcome::Completed(Ok(None)) => Ok(None),
            Outcome::Completed(Err(err)) => {
                handle_round_error(lobby, err)?;
                Ok(None)
            }
            Outcome::Stopped(abort_msg) => {
                handle_abort(lobby, &abort_msg)?;
                Ok(None)
            }
        }
    }

    /// Phase 3: deals roles and runs a full Secret Hitler match to completion (or host `!abort`).
    /// Stores the post-game [`GameReveal`] for the next `!reveal`.
    pub async fn play_round(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        round: SecretHitlerRound,
    ) -> Result<()> {
        self.pending_reveal = None;

        let setup = match GameSetup::random(round.players) {
            Ok(setup) => setup,
            Err(err) => return handle_round_error(lobby, err),
        };

        let host_for_busy = round.host.clone();
        let lobby_for_busy = lobby.clone();
        let busy_responder = lobby.inbox().on_map(move |msg| {
            if msg.is_cmd("!start") {
                let hint = if host_for_busy.is_online() && lobby_for_busy.has_user(&host_for_busy) {
                    format!("{} can type !abort", host_for_busy.nick())
                } else {
                    "type !abort".to_owned()
                };
                let _ = lobby_for_busy.say(
                    format_args!(
                        "⏳ A Secret Hitler game is already in progress ({hint} to cancel)!"
                    )
                    .yellow(),
                );
            } else if msg.is_cmd("!reveal") {
                let _ = msg.reply(
                    "⏳ Finish the current game first — !reveal is available after the game ends!"
                        .yellow(),
                );
            }
            None::<ChatMessage>
        });

        let outcome = run_dealt_game(bot, lobby, setup, &self.config)
            .until(abort_stop_listener(bot, lobby, &round.host))
            .or_until(busy_responder)
            .await;

        match outcome {
            Outcome::Completed(Ok(summary)) => {
                self.pending_reveal = Some(summary.reveal);
                lobby.say(irc_format!(
                    "🏁 {} Discuss what happened, then type {} for the full reveal — or {} for another game!",
                    "Game complete!".bold().green(),
                    "!reveal".bold().yellow(),
                    "!start".bold().cyan(),
                ))?;
            }
            Outcome::Completed(Err(err)) => handle_round_error(lobby, err)?,
            Outcome::Stopped(abort_msg) => handle_abort(lobby, &abort_msg)?,
        }

        Ok(())
    }
}

/// Standalone Secret Hitler lobby loop (`!start`, `!rules`, `!tracks`, `!reveal`, `!abort`).
pub async fn run_lobby_bot(bot: &Bot, lobby: &Room, config: &GameConfig) -> Result<()> {
    let mut state = SecretHitlerLobby::new(config.clone());
    loop {
        let launch = match state
            .wait_for_start(lobby)
            .until(bot.shutdown_requested())
            .await
        {
            Outcome::Completed(res) => res?,
            Outcome::Stopped(()) => break,
        };
        let Some(round) = (match state
            .recruit(bot, lobby, launch)
            .until(bot.shutdown_requested())
            .await
        {
            Outcome::Completed(res) => res?,
            Outcome::Stopped(()) => break,
        }) else {
            continue;
        };
        match state
            .play_round(bot, lobby, round)
            .until(bot.shutdown_requested())
            .await
        {
            Outcome::Completed(res) => res?,
            Outcome::Stopped(()) => break,
        }
    }
    Ok(())
}

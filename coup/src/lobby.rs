//! Stateful three-phase lobby (`CoupLobby`), standalone loop (`run_lobby_bot`), pre-game
//! recruitment (`recruit_players`), and `!rules` / `!roles` catalog commands.

use ircgm::prelude::*;
use tokio::time::Instant;

use crate::game::{run_live_game, GameConfig, GameReveal};
use crate::state::{copies_per_role, parse_role, GameMode, Role, MAX_PLAYERS, MIN_PLAYERS};

pub fn format_setup_note(player_count: usize, mode: GameMode) -> Option<Text> {
    let copies = copies_per_role(player_count)?;
    if player_count == 2 {
        let extra = if mode.reformation {
            " · opposite Allegiances"
        } else {
            ""
        };
        return Some(irc_format!(
            "{}{extra}",
            "2p variant: choose 1 card + 1 random (3-card deck), 1c/2c start".grey()
        ));
    }
    let extra = if mode.reformation {
        " · alternating Allegiances"
    } else {
        ""
    };
    Some(irc_format!(
        "{}-card deck ({copies}x each){extra}",
        copies * 5
    ))
}

pub fn send_lobby_help(lobby: &Room) -> Result<()> {
    let sep = " · ".grey();
    lobby.say(irc_format!(
        "👑 {} {} — {}{sep}{}{sep}{}{sep}{}{sep}{}",
        "Coup".bold().magenta(),
        "(2–10p)".grey(),
        "!start [inquisition|reformation]".bold().green(),
        "!rules".bold().cyan(),
        "!roles [name]".bold().cyan(),
        "!reveal".bold().yellow(),
        "!help".bold(),
    ))
}

pub fn send_open_lobby_help(reply_to: &ChatMessage, host: &User) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "👑 {} — {}{sep}{}{sep}{}{sep}{}{sep}{}",
        "Open Lobby".bold().yellow(),
        "!join / !play".bold().green(),
        "!leave".bold(),
        "!players".bold().cyan(),
        "!rules".bold().cyan(),
        "!roles [name]".bold().cyan(),
    ))?;
    reply_to.reply(irc_format!(
        "  {} {}{sep}{}{sep}{}",
        format!("Host ({}):", host.nick()).bold(),
        "!mode <classic|inquisition|reformation>".bold().cyan(),
        "!go".bold().green(),
        "!abort".bold().orange(),
    ))
}

pub fn send_active_game_help(reply_to: &ChatMessage, mode: GameMode) -> Result<()> {
    let sep = " · ".grey();
    let mut actions = vec![
        "!income".bold(),
        "!aid".bold(),
        "!tax".bold().magenta(),
        "!steal <nick>".bold().cyan(),
        "!assassinate <nick>".bold().orange(),
        "!exchange".bold().green(),
        "!coup <nick>".bold().red(),
    ];
    if mode.inquisitor {
        actions.push("!examine <nick>".bold().yellow());
    }
    if mode.reformation {
        actions.push("!convert [nick]".bold().cyan());
        actions.push("!embezzle".bold().yellow());
    }
    reply_to.reply(irc_format!(
        "👑 {} ({}) — Turn: {}",
        "Active Game".bold().yellow(),
        mode.styled_label(),
        Text::join(&actions, sep.clone())
    ))?;
    reply_to.reply(irc_format!(
        "  Reactions: {} / {} / {}{sep}Info: {} / {} / {} (DM) / {} / {}{sep}Cancel: {}",
        "!challenge".bold().red(),
        "!block [role]".bold().yellow(),
        "!pass".bold().green(),
        "!board".bold().cyan(),
        "!players".bold().cyan(),
        "!hand".bold().cyan(),
        "!roles".bold().cyan(),
        "!time".bold().cyan(),
        "!abort".bold().orange(),
    ))
}

pub fn send_rules_summary(reply_to: &ChatMessage, mode: Option<GameMode>) -> Result<()> {
    reply_to.reply(irc_format!(
        "📜 {} — 2 hidden influence cards, 2 coins (2p: 1c/2c + pick 1 card). Last survivor wins! Actions: {} (+1c), {} (+2c, Duke blocks), {} (7c, unblockable; mandatory at 10+c).",
        "Coup Rules".bold().magenta(), "Income".bold(), "Foreign Aid".bold(), "Coup".bold().red(),
    ))?;
    reply_to.reply(irc_format!(
        "  {}: claim any character (bluffing allowed!). Anyone may {} a character action or block — loser loses 1 influence; proven card swaps into Court Deck. Assassinate (3c) refunds if challenged & failed, stays spent if blocked!",
        "Bluff & Challenge".bold(), "!challenge".bold().red(),
    ))?;
    if mode.is_none_or(|m| m.inquisitor) {
        reply_to.reply(irc_format!(
            "  {} (!start inquisition): replaces Ambassador — Exchange draws 1 card, or {} peeks a target's chosen card and lets you keep or force-swap it; blocks Steal.",
            "Inquisitor".bold().yellow(), "!examine <nick>".bold().yellow(),
        ))?;
    }
    if mode.is_none_or(|m| m.reformation) {
        reply_to.reply(irc_format!(
            "  {} (!start reformation): Loyalist vs Reformist — cannot attack or block Foreign Aid of same Allegiance unless all survivors match (challenges always allowed!). {} (1c self / 2c other → Treasury) · {} takes Treasury by claiming NOT to hold Duke!",
            "Reformation".bold().orange(), "!convert [nick]".bold().cyan(), "!embezzle".bold().yellow(),
        ))?;
    }
    Ok(())
}

pub fn send_roles_catalog(
    reply_to: &ChatMessage,
    raw_filter: Option<&str>,
    mode: Option<GameMode>,
) -> Result<()> {
    if let Some(q) = raw_filter.map(str::trim).filter(|s| !s.is_empty()) {
        return match parse_role(q) {
            Some(role) => reply_to.reply(irc_format!("🃏 {} — {}", role.styled(), role.summary())),
            None => reply_to.reply(format_args!("Unknown role '{q}' — try Duke, Assassin, Captain, Ambassador, Inquisitor, or Contessa.").yellow()),
        };
    }
    let (header, roles): (Text, Vec<Role>) = match mode {
        Some(m) => (
            irc_format!("🃏 {} —", m.styled_label()),
            m.active_roles().to_vec(),
        ),
        None => (
            irc_format!("🃏 {} —", "Coup Characters".bold().magenta()),
            Role::ALL.to_vec(),
        ),
    };
    reply_to.reply(header)?;
    for role in roles {
        reply_to.reply(irc_format!("  {} — {}", role.styled(), role.summary()))?;
    }
    Ok(())
}

pub fn handle_catalog_query(
    _room: &Room,
    msg: &ChatMessage,
    mode: Option<GameMode>,
) -> Result<bool> {
    if msg.is_cmd("!rules") {
        send_rules_summary(msg, mode)?;
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_args("!roles").or_else(|| msg.cmd_args("!role")) {
        send_roles_catalog(msg, Some(arg), mode)?;
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
            players[0].nick().bold().yellow()
        ))?;
    }
    Ok(false)
}

fn handle_mode_cmd(
    room: &Room,
    msg: &ChatMessage,
    is_host: bool,
    arg: &str,
    mode: &mut GameMode,
) -> Result<()> {
    if !is_host {
        return msg.reply("Only the lobby host can change the game mode!".yellow());
    }
    let t = arg.trim();
    if t.is_empty() {
        return msg.reply(irc_format!(
            "Current mode: {} — switch with {} (classic, inquisition, reformation, or reformation inquisition).",
            mode.styled_label(),
            "!mode <mode>".bold().cyan(),
        ));
    }
    match GameMode::parse(t) {
        Some(next) => {
            *mode = next;
            room.say(irc_format!("⚙️ Mode switched to {}!", mode.styled_label()))
        }
        None => msg.reply(format_args!("Unknown mode '{t}' — try classic, inquisition, reformation, or reformation inquisition!").yellow()),
    }
}

pub async fn recruit_players(
    room: &Room,
    host: User,
    mut mode: GameMode,
    config: &GameConfig,
) -> Result<Option<(Vec<User>, GameMode)>> {
    let mut inbox = room.inbox();
    let mut players = vec![host.clone()];
    let sep = " · ".grey();
    room.say(irc_format!(
        "👑 {} opened a {} lobby {}! Type {} (or {}) to play ({}s).",
        host.nick().bold().yellow(),
        mode.styled_label(),
        "(2–10p)".grey(),
        "!join".bold().green(),
        "!play".bold().green(),
        config.lobby_duration.as_secs(),
    ))?;
    room.say(irc_format!(
        "  {} {}{sep}{}{sep}{}{sep}{}",
        "Host:".bold(),
        "!mode <classic|inquisition|reformation>".bold().cyan(),
        "!go".bold().green(),
        "!abort".bold().orange(),
        "!roles".bold().cyan(),
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
                            let detail = format_setup_note(n, mode).unwrap_or_else(|| {
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
                        let suffix = format_setup_note(n, mode).map_or_else(
                            || Text::from(format!("(need {MIN_PLAYERS}+ to start)")),
                            |note| irc_format!("({note})"),
                        );
                        msg.reply(irc_format!(
                            "👥 {} {} — {list} {suffix}",
                            format!("Lobby ({n}/{MAX_PLAYERS}):").bold(),
                            mode.styled_label()
                        ))?;
                    } else if msg.is_cmd("!help") {
                        send_open_lobby_help(&msg, &players[0])?;
                    } else if msg.is_cmd("!reveal") {
                        msg.reply("⏳ Finish the current game first — !reveal is available after the game ends!".yellow())?;
                    } else if handle_catalog_query(room, &msg, Some(mode))? {
                        continue;
                    } else if let Some(arg) = msg
                        .cmd_args("!mode")
                        .or_else(|| msg.cmd_args("!preset"))
                        .or_else(|| msg.cmd_args("!variant"))
                    {
                        handle_mode_cmd(room, &msg, is_host, arg, &mut mode)?;
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
                                    "Need at least {MIN_PLAYERS} players for Coup (currently {}).",
                                    players.len()
                                )
                                .yellow(),
                            )?;
                        } else {
                            break;
                        }
                    } else if let Some(arg) = msg.cmd_args("!start") {
                        let t = arg.trim();
                        let reply = if is_host {
                            let hint = if !t.is_empty() && GameMode::parse(t).is_some() {
                                format!("!mode {t}")
                            } else {
                                "!mode <classic|inquisition|reformation>".into()
                            };
                            irc_format!("Lobby is already open — use {} to change mode, {} to start now, or {} to cancel.", hint.bold().cyan(), "!go".bold().green(), "!abort".bold().orange())
                        } else if players.contains(&msg.sender) {
                            irc_format!("Lobby is already open (hosted by {}) — waiting for more players or {}!", players[0].nick().bold(), "!go".bold().green())
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
            room.say(format_args!("Player count ({}) outside required {MIN_PLAYERS}–{MAX_PLAYERS} range for Coup — lobby closed.", players.len()).yellow())?;
        }
        return Ok(None);
    }
    Ok(Some((players, mode)))
}

#[derive(Debug)]
pub struct CoupLaunch {
    pub starter: User,
    pub mode: GameMode,
}

#[derive(Debug)]
pub struct CoupRound {
    pub host: User,
    pub players: Vec<User>,
    pub mode: GameMode,
}

#[derive(Debug)]
pub struct CoupLobby {
    config: GameConfig,
    pending_reveal: Option<GameReveal>,
    announced_help: bool,
}

fn abort_stop_listener(
    bot: &Bot,
    lobby: &Room,
    host: &User,
) -> impl std::future::Future<Output = ChatMessage> + Send + 'static {
    let inbox = bot.inbox_scope(InboxScope::new().room(lobby).dm_from(host));
    let (host, lobby) = (host.clone(), lobby.clone());
    async move {
        inbox
            .on_map(move |m| {
                let host_gone = !host.is_online() || !lobby.has_user(&host);
                (m.is_cmd("!abort") && (m.sender == host || (host_gone && m.is_in(&lobby))))
                    .then(|| m.clone())
            })
            .await
    }
}

fn handle_round_error(lobby: &Room, err: Error) -> Result<()> {
    let lost_lobby = matches!(&err, Error::BotLeftRoom { channel, .. } if lobby.casemap().str_eq(channel, lobby.name()));
    if err.is_fatal() || lost_lobby {
        return Err(err);
    }
    tracing::warn!(error = %err, "Coup game ended early");
    lobby.say(format_args!("⚠️ Coup game ended early: {err} — lobby is open again!").yellow())
}

fn handle_abort(lobby: &Room, abort_msg: &ChatMessage) -> Result<()> {
    if abort_msg.is_dm() {
        abort_msg.reply("🛑 Aborted Coup game.".yellow())?;
    }
    lobby.say(irc_format!(
        "🛑 {} {}",
        abort_msg.sender.nick().bold(),
        "aborted the Coup game — lobby is open!".yellow()
    ))
}

impl CoupLobby {
    pub fn new(config: GameConfig) -> Self {
        Self {
            config,
            pending_reveal: None,
            announced_help: false,
        }
    }

    pub async fn wait_for_start(&mut self, lobby: &Room) -> Result<CoupLaunch> {
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
            } else if handle_catalog_query(lobby, &msg, None)? {
                continue;
            } else if let Some(arg) = msg.cmd_args("!start") {
                match GameMode::parse(arg) {
                    Some(mode) => return Ok(CoupLaunch { starter: msg.sender.clone(), mode }),
                    None => msg.reply(format_args!("Unknown mode '{}' — try !start, !start inquisition, !start reformation, or !start reformation inquisition!", arg.trim()).yellow())?,
                }
            }
        }
    }

    pub async fn recruit(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        launch: CoupLaunch,
    ) -> Result<Option<CoupRound>> {
        match recruit_players(lobby, launch.starter.clone(), launch.mode, &self.config)
            .until(abort_stop_listener(bot, lobby, &launch.starter))
            .await
        {
            Outcome::Completed(Ok(Some((players, mode)))) => Ok(Some(CoupRound {
                host: players[0].clone(),
                players,
                mode,
            })),
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

    pub async fn play_round(&mut self, bot: &Bot, lobby: &Room, round: CoupRound) -> Result<()> {
        self.pending_reveal = None;
        let (host, room) = (round.host.clone(), lobby.clone());
        let busy_responder = lobby.inbox().on_map(move |msg| {
            if msg.is_cmd("!start") {
                let hint = if host.is_online() && room.has_user(&host) {
                    format!("{} can type !abort", host.nick())
                } else {
                    "type !abort".to_owned()
                };
                let _ = room.say(
                    format_args!("⏳ A Coup game is already in progress ({hint} to cancel)!")
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

        match run_live_game(bot, lobby, round.players, round.mode, &self.config)
            .until(abort_stop_listener(bot, lobby, &round.host))
            .or_until(busy_responder)
            .await
        {
            Outcome::Completed(Ok(summary)) => {
                self.pending_reveal = Some(summary.reveal);
                lobby.say(irc_format!(
                    "🏁 {} Discuss what happened, then type {} for the full chronicle — or {} for another game!",
                    "Game complete!".bold().green(), "!reveal".bold().yellow(), "!start [inquisition|reformation]".bold().cyan(),
                ))?;
            }
            Outcome::Completed(Err(err)) => handle_round_error(lobby, err)?,
            Outcome::Stopped(abort_msg) => handle_abort(lobby, &abort_msg)?,
        }
        Ok(())
    }
}

pub async fn run_lobby_bot(bot: &Bot, lobby: &Room, config: &GameConfig) -> Result<()> {
    let mut state = CoupLobby::new(config.clone());
    loop {
        let Outcome::Completed(launch) = state
            .wait_for_start(lobby)
            .until(bot.shutdown_requested())
            .await
        else {
            break;
        };
        let Outcome::Completed(round) = state
            .recruit(bot, lobby, launch?)
            .until(bot.shutdown_requested())
            .await
        else {
            break;
        };
        if let Some(round) = round? {
            let Outcome::Completed(res) = state
                .play_round(bot, lobby, round)
                .until(bot.shutdown_requested())
                .await
            else {
                break;
            };
            res?;
        }
    }
    Ok(())
}

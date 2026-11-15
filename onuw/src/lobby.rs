//! Persistent lobby supervisor (`run_lobby_bot`), pre-game recruitment (`recruit_players`),
//! and `!roles` / `!scenarios` catalog commands.

use futures::future::try_join_all;
use futures::FutureExt as _;
use ircgm::prelude::*;
use tokio::time::Instant;

use crate::dawn::GameReveal;
use crate::day::is_active_in_room;
use crate::deck::{parse_role_spec_tokens, DeckSpec};
use crate::game::{run_single_game, GameConfig, PlayOutcome, MAX_PLAYERS, MIN_PLAYERS};
use crate::role::{Role, GameSet, WakePhase};
use crate::roles::{find_role, roles_in_game, ALL_ROLES};
use crate::scenarios::{
    all_scenarios, find_scenario, parse_player_count_filter, scenario_count, scenarios_in_pack,
    Scenario, ScenarioPack,
};

pub fn is_admin_msg(msg: &ChatMessage, admins: &[String]) -> bool {
    let Some(acct) = msg.account.as_deref() else {
        return false;
    };
    admins.iter().any(|a| a.eq_ignore_ascii_case(acct))
}

enum StopReason {
    Shutdown,
    Admin(Box<ChatMessage>),
    Aborted(Box<ChatMessage>),
}

// ============================================================================
// Catalog & Help Formatters (`!help`, `!scenarios`, `!scenario`, `!roles`, `!role`)
// ============================================================================

pub fn send_lobby_help(lobby: &Room) -> Result<()> {
    lobby.say(irc_format!(
        "🐺 {} — {} opens a lobby | {} | {} | {} | {} | {} | {} cancels.",
        "One Night Ultimate Werewolf".bold().yellow(),
        "!onuw [pack|scenario]".bold().green(),
        "!reveal".bold().yellow(),
        "!scenarios [pack|Np]".bold().cyan(),
        "!scenario <id|name>".bold().cyan(),
        "!roles [game]".bold().cyan(),
        "!role <name>".bold().cyan(),
        "!abort".bold().orange(),
    ))
}

pub fn send_packs_info(room: &Room) -> Result<()> {
    let packs: Vec<String> = ScenarioPack::ALL
        .iter()
        .map(|&pack| {
            format!(
                "`{}` ({}, {} scenarios)",
                pack.slug(),
                pack.label(),
                scenarios_in_pack(pack).len()
            )
        })
        .collect();
    room.say(irc_format!(
        "📜 {} ({} total): {} — browse with `!scenarios <pack>` / `!scenarios <Np>` or `!scenario <id>`!",
        "Scenario Packs".bold().cyan(),
        scenario_count(),
        packs.join(" | "),
    ))
}

pub fn send_single_scenario_info(reply_to: &ChatMessage, scen: &'static Scenario) -> Result<()> {
    for line in scen.describe_lines() {
        reply_to.reply(line)?;
    }
    Ok(())
}

pub fn send_scenarios_catalog(reply_to: &ChatMessage, raw_filter: Option<&str>) -> Result<()> {
    let filter = raw_filter.map(str::trim).filter(|s| !s.is_empty());
    let Some(q) = filter else {
        let packs: Vec<String> = ScenarioPack::ALL
            .iter()
            .map(|&pack| {
                format!(
                    "`{}` ({}: {})",
                    pack.slug(),
                    pack.label(),
                    scenarios_in_pack(pack).len()
                )
            })
            .collect();
        reply_to.reply(irc_format!(
            "📜 {} ({} total, ordered by pack): {}",
            "Preset Scenarios".bold().cyan(),
            scenario_count(),
            packs.join(" | "),
        ))?;
        return reply_to.reply(
            "Use `!scenarios <pack>` (e.g. `base`, `daybreak`, `vampire`, `epic_battle`, `alien`, `villains`), `!scenarios <Np>` (e.g. `5p`), or `!scenario <id|name>`!",
        );
    };

    let mut pack_filter: Option<ScenarioPack> = None;
    let mut count_filter: Option<usize> = None;
    let mut unmatched_tokens = false;
    for tok in q.split_whitespace() {
        if let Some(p) = ScenarioPack::parse(tok) {
            pack_filter = Some(p);
        } else if let Some(n) = parse_player_count_filter(tok) {
            count_filter = Some(n);
        } else {
            unmatched_tokens = true;
        }
    }

    if unmatched_tokens || (pack_filter.is_none() && count_filter.is_none()) {
        if let Some(scen) = find_scenario(q) {
            return send_single_scenario_info(reply_to, scen);
        }
        return reply_to.reply(
            format_args!(
                "Unknown pack or scenario `{q}`. Packs: `base`, `daybreak`, `vampire`, `epic_battle`, `alien`, `villains` (or e.g. `!scenarios 6p`)."
            )
            .yellow(),
        );
    }

    let matched: Vec<&'static Scenario> = all_scenarios()
        .filter(|s| pack_filter.is_none_or(|p| s.pack == p))
        .filter(|s| count_filter.is_none_or(|n| s.supports_player_count(n)))
        .collect();

    if matched.is_empty() {
        return reply_to.reply("No scenarios match that filter.".yellow());
    }

    let mut current_group = String::new();
    let mut items_in_group: Vec<String> = Vec::new();
    let flush_group = |group: &str, items: &mut Vec<String>| -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        for chunk in items.chunks(5) {
            reply_to.reply(irc_format!(
                "  • {}: {}",
                group.bold().cyan(),
                chunk.join(" | "),
            ))?;
        }
        items.clear();
        Ok(())
    };

    for scen in matched {
        let group = if pack_filter.is_some() {
            scen.section.to_owned()
        } else {
            scen.pack.label().to_owned()
        };
        if group != current_group {
            flush_group(&current_group, &mut items_in_group)?;
            current_group = group;
        }
        items_in_group.push(format!(
            "`{}` ({}, {}, {})",
            scen.id,
            scen.name,
            scen.difficulty.label(),
            scen.range_label()
        ));
    }
    flush_group(&current_group, &mut items_in_group)?;
    reply_to.reply("Type `!scenario <id>` for full cards or `!onuw <id>` / `!preset <id>` to play!")
}

pub fn send_roles_catalog(reply_to: &ChatMessage, raw_filter: Option<&str>) -> Result<()> {
    let filter = raw_filter.map(str::trim).filter(|s| !s.is_empty());
    if let Some(q) = filter {
        if let Some(gs) = GameSet::parse(q) {
            let list: Vec<String> = roles_in_game(gs)
                .into_iter()
                .map(|r| r.name().to_owned())
                .collect();
            return reply_to.reply(irc_format!(
                "🃏 {} ({} roles): {}",
                gs.label().bold().cyan(),
                list.len(),
                list.join(", "),
            ));
        }
        if let Some(role) = find_role(q) {
            return send_single_role_info(reply_to, role);
        }
        return reply_to.reply(
            format_args!(
                "Unknown game or role `{q}`. Games: `base`, `daybreak`, `vampire`, `alien`, `villains`."
            )
            .yellow(),
        );
    }

    for gs in GameSet::ALL {
        let names: Vec<&'static str> = roles_in_game(gs).into_iter().map(|r| r.name()).collect();
        reply_to.reply(irc_format!(
            "  • {}: {}",
            gs.label().bold().cyan(),
            names.join(", "),
        ))?;
    }
    reply_to.reply(format_args!(
        "Total: {} roles — type `!role <name>` for details!",
        ALL_ROLES.len()
    ))
}

pub fn send_single_role_info(reply_to: &ChatMessage, role: &'static dyn Role) -> Result<()> {
    let wake_str = match role.wake() {
        None => "Sleeps".to_owned(),
        Some(w) => match w.phase {
            WakePhase::PreCopycatDusk | WakePhase::Copycat | WakePhase::Dusk => {
                format!("Dusk #{}", w.order)
            }
            WakePhase::Night => format!("Night #{}", w.order),
        },
    };
    reply_to.reply(irc_format!(
        "🃏 {} [{} | {} | {}]: {}",
        role.styled_name(),
        role.game().label(),
        role.team().label(),
        wake_str,
        role.night_summary(),
    ))
}

/// Shared catalog command handler used in both idle and open recruitment lobbies.
pub fn handle_catalog_query(room: &Room, msg: &ChatMessage) -> Result<bool> {
    if let Some(arg) = msg.cmd_args("!presets") {
        let q = arg.trim();
        if q.is_empty() {
            send_packs_info(room)?;
        } else {
            send_scenarios_catalog(msg, Some(q))?;
        }
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_args("!scenarios") {
        send_scenarios_catalog(msg, Some(arg))?;
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_args("!scenario") {
        let q = arg.trim();
        if q.is_empty() {
            send_scenarios_catalog(msg, None)?;
        } else {
            match find_scenario(q) {
                Some(scen) => send_single_scenario_info(msg, scen)?,
                None => {
                    msg.reply(format_args!("Unknown scenario `{q}` (try `!scenarios`).").yellow())?
                }
            }
        }
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_args("!roles") {
        send_roles_catalog(msg, Some(arg))?;
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_arg("!role").or_else(|| msg.cmd_arg("!info")) {
        match find_role(arg) {
            Some(r) => send_single_role_info(msg, r)?,
            None => msg.reply(format_args!("Unknown role `{arg}` (try `!roles`).").yellow())?,
        }
        return Ok(true);
    }
    Ok(false)
}

// ============================================================================
// Pre-Game Recruitment Lobby (`!join`, `!leave`, `!preset`, `!add`, `!remove`, `!go`)
// ============================================================================

pub async fn recruit_players(
    room: &Room,
    host: User,
    mut deck_spec: DeckSpec,
    config: &GameConfig,
) -> Result<Option<(Vec<User>, DeckSpec)>> {
    let mut inbox = room.inbox();
    let mut players: Vec<User> = vec![host.clone()];

    room.say(irc_format!(
        "🌕 {} opened an ONUW [{}] lobby! Type {}, {}, {}, {}, or {} ({}s).",
        host.nick().bold().yellow(),
        deck_spec.mode_label().as_str().bold().cyan(),
        "!join".bold().green(),
        "!preset <pack|scenario>".bold(),
        "!add <role>".bold(),
        "!remove <role>".bold(),
        "!go".bold().cyan(),
        config.lobby_duration.as_secs(),
    ))?;

    let deadline = Instant::now() + config.lobby_duration;
    let recruit_loop = async {
        loop {
            let (_min_cap, max_cap) = deck_spec.player_range();
            if players.len() >= max_cap.min(MAX_PLAYERS) {
                break;
            }
            match inbox.next_event().await? {
                Event::Chat(msg) => {
                    let is_host = players.first() == Some(&msg.sender);

                    if msg.is_cmd("!join") || msg.is_cmd("!in") {
                        let (_, cur_max) = deck_spec.player_range();
                        if players.len() >= cur_max {
                            msg.reply(
                                format_args!(
                                    "Lobby is full for `{}` (max {cur_max} players)!",
                                    deck_spec.mode_label()
                                )
                                .yellow(),
                            )?;
                            continue;
                        }
                        if !players.contains(&msg.sender) {
                            players.push(msg.sender.clone());
                            room.say(irc_format!(
                                "{} joined! ({}/{cur_max})",
                                msg.sender.nick().bold().green(),
                                players.len(),
                            ))?;
                        }
                    } else if msg.is_cmd("!leave") || msg.is_cmd("!out") {
                        let was_host = is_host;
                        let before = players.len();
                        players.retain(|u| *u != msg.sender);
                        if players.is_empty() {
                            room.say("Lobby emptied and closed.".yellow())?;
                            break;
                        } else if players.len() < before && was_host {
                            room.say(irc_format!(
                                "{} left — {} is now lobby host!",
                                msg.sender.nick().bold(),
                                players[0].nick().bold().yellow(),
                            ))?;
                        }
                    } else if msg.is_cmd("!players") || msg.is_cmd("!who") {
                        let (_, cur_max) = deck_spec.player_range();
                        let list: Vec<String> = players.iter().map(User::nick).collect();
                        msg.reply(format!(
                            "👥 Lobby ({}/{cur_max}): {}",
                            players.len(),
                            list.join(", ")
                        ))?;
                    } else if msg.is_cmd("!deck") {
                        msg.reply(format!("🃏 {}", deck_spec.preview_for_players(players.len())))?;
                    } else if handle_catalog_query(room, &msg)? {
                        continue;
                    } else if let Some(arg) =
                        msg.cmd_args("!preset").or_else(|| msg.cmd_args("!setscenario"))
                    {
                        if !is_host {
                            msg.reply("Only the lobby host can change the deck!".yellow())?;
                            continue;
                        }
                        let q = arg.trim();
                        if q.is_empty() {
                            send_packs_info(room)?;
                            continue;
                        }
                        if let Some(pack) = ScenarioPack::parse(q) {
                            deck_spec.set_pack(pack);
                            room.say(irc_format!(
                                "📦 Deck pack switched to {}! ({})",
                                pack.label().bold().yellow(),
                                deck_spec.preview_for_players(players.len()),
                            ))?;
                        } else if let Some(scen) = find_scenario(q) {
                            if players.len() > scen.max_players() {
                                msg.reply(
                                    format_args!(
                                        "Scenario `{}` supports {} players, but {} are already in the lobby!",
                                        scen.id,
                                        scen.range_label(),
                                        players.len()
                                    )
                                    .yellow(),
                                )?;
                                continue;
                            }
                            deck_spec.set_scenario(scen);
                            room.say(irc_format!(
                                "📜 Scenario switched to {} (`{}`)! ({})",
                                scen.name.bold().yellow(),
                                scen.id,
                                deck_spec.preview_for_players(players.len()),
                            ))?;
                        } else {
                            msg.reply(
                                format_args!(
                                    "Unknown pack or scenario `{q}` — try `!scenarios`!"
                                )
                                .yellow(),
                            )?;
                        }
                    } else if let Some(arg) = msg.cmd_args("!add") {
                        if !is_host {
                            msg.reply("Only the lobby host can customize the deck!".yellow())?;
                            continue;
                        }
                        match parse_role_spec_tokens(arg) {
                            Ok(specs) => {
                                let mut summary_parts = Vec::new();
                                for (role, count) in specs {
                                    let added_n = deck_spec.add_role(role, count);
                                    if added_n > 1 {
                                        summary_parts.push(format!("{added_n}x {}", role.name()));
                                    } else {
                                        summary_parts.push(role.name().to_owned());
                                    }
                                }
                                room.say(irc_format!(
                                    "➕ Added {} — {}",
                                    summary_parts.join(", ").bold().green(),
                                    deck_spec.preview_for_players(players.len()),
                                ))?;
                            }
                            Err(e) => msg.reply(e.yellow())?,
                        }
                    } else if let Some(arg) =
                        msg.cmd_args("!remove").or_else(|| msg.cmd_args("!rm"))
                    {
                        if !is_host {
                            msg.reply("Only the lobby host can customize the deck!".yellow())?;
                            continue;
                        }
                        match parse_role_spec_tokens(arg) {
                            Ok(specs) => {
                                let mut summary_parts = Vec::new();
                                for (role, count) in specs {
                                    let rem_n = deck_spec.remove_role(role, count);
                                    if rem_n > 1 {
                                        summary_parts.push(format!("{rem_n}x {}", role.name()));
                                    } else {
                                        summary_parts.push(role.name().to_owned());
                                    }
                                }
                                room.say(irc_format!(
                                    "➖ Removed {} — {}",
                                    summary_parts.join(", ").bold().orange(),
                                    deck_spec.preview_for_players(players.len()),
                                ))?;
                            }
                            Err(e) => msg.reply(e.yellow())?,
                        }
                    } else if msg.is_cmd("!clear") {
                        if !is_host {
                            msg.reply("Only the lobby host can customize the deck!".yellow())?;
                            continue;
                        }
                        deck_spec.clear_to_custom();
                        room.say(
                            "🧹 Switched to pure Custom Deck! Add roles with `!add <role>` (unfilled slots default to Villager)."
                                .cyan(),
                        )?;
                    } else if msg.is_cmd("!reset") {
                        if !is_host {
                            msg.reply("Only the lobby host can customize the deck!".yellow())?;
                            continue;
                        }
                        deck_spec.reset_overrides();
                        room.say(format!(
                            "🔄 Reset custom overrides — {}",
                            deck_spec.preview_for_players(players.len())
                        ))?;
                    } else if msg.is_cmd("!go") || msg.is_cmd("!force") {
                        if !is_host {
                            msg.reply("Only the lobby host can start early with `!go`!".yellow())?;
                            continue;
                        }
                        let (min_req, max_req) = deck_spec.player_range();
                        if players.len() < min_req.max(MIN_PLAYERS) {
                            msg.reply(
                                format_args!(
                                    "Need at least {} players for `{}` (currently {}).",
                                    min_req.max(MIN_PLAYERS),
                                    deck_spec.mode_label(),
                                    players.len()
                                )
                                .yellow(),
                            )?;
                            continue;
                        }
                        if players.len() > max_req {
                            msg.reply(
                                format_args!(
                                    "`{}` allows at most {max_req} players (currently {}).",
                                    deck_spec.mode_label(),
                                    players.len()
                                )
                                .yellow(),
                            )?;
                            continue;
                        }
                        if let Err(err) = deck_spec.build_for_players(players.len()) {
                            msg.reply(err.yellow())?;
                            continue;
                        }
                        break;
                    }
                }
                Event::Parted { user, .. }
                | Event::Kicked { user, .. }
                | Event::Quit { user, .. } => {
                    players.retain(|u| *u != user);
                    if players.is_empty() {
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    };

    recruit_loop.run_until_deadline(deadline).await?;

    players.retain(|u| is_active_in_room(room, u));
    let (min_req, max_req) = deck_spec.player_range();
    let min_needed = min_req.max(MIN_PLAYERS);
    if players.len() < min_needed || players.len() > max_req {
        if !players.is_empty() {
            room.say(
                format_args!(
                    "Player count ({}) outside required {min_needed}–{max_req} range for `{}` — lobby closed.",
                    players.len(),
                    deck_spec.mode_label()
                )
                .yellow(),
            )?;
        }
        return Ok(None);
    }

    if let Some(dm_dur) = config.dm_check_timeout {
        let check_deadline = Instant::now() + dm_dur;
        room.say(format_args!(
            "📩 DM check: reply `ok` in DM within {}s to confirm you can receive secret cards!",
            dm_dur.as_secs()
        ))?;
        let check_cfg = AskConfig::new()
            .reply_deadline(check_deadline)
            .no_timeout_message();
        let checks = try_join_all(players.iter().map(|p| {
            let cfg = &check_cfg;
            async move {
                let ans = p
                    .ask("👋 Reply `ok` to confirm DMs for ONUW!", |m| {
                        let t = m.text.trim();
                        (t.eq_ignore_ascii_case("ok")
                            || t.eq_ignore_ascii_case("ready")
                            || m.is_cmd("!ok"))
                        .then(|| AskMatch::accept_say((), "✔ Confirmed!".green()))
                        .unwrap_or_else(AskMatch::ignore)
                    })
                    .config(cfg)
                    .await?;
                Ok::<_, Error>((p.clone(), ans))
            }
        }))
        .await?;

        let mut verified = Vec::with_capacity(checks.len());
        let mut dropped_nicks = Vec::new();
        for (p, ans) in checks {
            if matches!(ans, Answer::Ok(())) && is_active_in_room(room, &p) {
                verified.push(p);
            } else {
                dropped_nicks.push(p.nick());
            }
        }

        if !dropped_nicks.is_empty() {
            room.say(
                format_args!(
                    "⚠️ Dropped from roster (unconfirmed/unreachable DMs): {}",
                    dropped_nicks.join(", ")
                )
                .yellow(),
            )?;
        }
        players = verified;
        if players.len() < min_needed || players.len() > max_req {
            room.say(
                format_args!(
                    "DM-verified player count ({}) outside required {min_needed}–{max_req} range — game cancelled.",
                    players.len()
                )
                .yellow(),
            )?;
            return Ok(None);
        }
    }

    while deck_spec.added.len() > players.len() + 3 {
        deck_spec.added.pop();
    }

    Ok(Some((players, deck_spec)))
}

// ============================================================================
// Persistent Multi-Game Lobby Supervisor (`run_lobby_bot`)
// ============================================================================

pub async fn run_lobby_bot(
    bot: &Bot,
    lobby: &Room,
    admins: &[String],
    config: &GameConfig,
) -> Result<()> {
    send_lobby_help(lobby)?;
    let mut pending_reveal: Option<GameReveal> = None;

    loop {
        let Some((starter, initial_spec)) = ({
            let mut inbox = lobby.inbox().lossy();
            loop {
                let idle_msg = match inbox.next_chat().until(bot.shutdown_requested()).await {
                    Outcome::Completed(res) => res?,
                    Outcome::Stopped(()) => break None,
                };

                if idle_msg.is_cmd("!help") {
                    send_lobby_help(lobby)?;
                    continue;
                }
                if idle_msg.is_cmd("!reveal") {
                    match pending_reveal.take() {
                        Some(reveal) => reveal.announce(lobby)?,
                        None => {
                            idle_msg.reply("Nothing left to reveal from the last game!".yellow())?
                        }
                    }
                    continue;
                }
                if handle_catalog_query(lobby, &idle_msg)? {
                    continue;
                }

                let Some(raw_args) = idle_msg
                    .cmd_args("!onuw")
                    .or_else(|| idle_msg.cmd_args("!play"))
                    .or_else(|| idle_msg.cmd_args("!start"))
                else {
                    continue;
                };

                let trimmed = raw_args.trim();
                let Some(spec) = DeckSpec::parse(trimmed) else {
                    idle_msg.reply(
                        format_args!(
                            "Unknown pack or scenario `{trimmed}` — try `!scenarios`!"
                        )
                        .yellow(),
                    )?;
                    continue;
                };

                pending_reveal = None;
                break Some((idle_msg.sender.clone(), spec));
            }
        }) else {
            break;
        };

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
            let starter_gone = !is_active_in_room(&lobby_for_abort, &starter_for_abort);
            if is_starter || (starter_gone && m.is_in(&lobby_for_abort)) {
                Some(StopReason::Aborted(Box::new(m.clone())))
            } else {
                None
            }
        });

        let starter_for_busy = starter.clone();
        let lobby_for_busy = lobby.clone();
        let busy_responder = lobby.inbox().on_map(move |msg| {
            if msg.is_cmd("!onuw") || msg.is_cmd("!play") || msg.is_cmd("!start") {
                let abort_hint = if is_active_in_room(&lobby_for_busy, &starter_for_busy) {
                    format!("{} can type `!abort`", starter_for_busy.nick())
                } else {
                    "type `!abort`".to_owned()
                };
                let _ = lobby_for_busy.say(
                    format_args!("⏳ An ONUW game is already in progress ({abort_hint} to cancel)!")
                        .yellow(),
                );
            } else if msg.is_cmd("!reveal") {
                let _ = msg.reply(
                    "⏳ Finish the current game first — `!reveal` is available after voting ends!"
                        .yellow(),
                );
            } else if msg.is_cmd("!help") {
                let _ = send_lobby_help(&lobby_for_busy);
            }
            None::<StopReason>
        });

        let outcome = run_single_game(bot, lobby, starter, initial_spec, config)
            .until(bot.shutdown_requested().map(|()| StopReason::Shutdown))
            .or_until(admin_stop)
            .or_until(abort_stop)
            .or_until(busy_responder)
            .await;

        match outcome {
            Outcome::Completed(Ok(PlayOutcome::Completed(reveal))) => {
                pending_reveal = Some(reveal);
                lobby.say(irc_format!(
                    "🏁 {} Discuss what happened, then type {} for the full reveal — or {} for another round!",
                    "Round complete!".bold().green(),
                    "!reveal".bold().yellow(),
                    "!onuw [pack|scenario]".bold().cyan(),
                ))?;
            }
            Outcome::Completed(Ok(PlayOutcome::NotEnoughPlayers)) => {}
            Outcome::Completed(Err(err)) => {
                let lost_lobby = matches!(
                    &err,
                    Error::BotLeftRoom { channel, .. } if lobby.casemap().str_eq(channel, lobby.name())
                );
                if err.is_fatal() || lost_lobby {
                    return Err(err);
                }
                tracing::warn!(error = %err, "ONUW game ended early");
                lobby.say(
                    format_args!("⚠️ ONUW game ended early: {err} — lobby is open again!").yellow(),
                )?;
            }
            Outcome::Stopped(StopReason::Shutdown) => break,
            Outcome::Stopped(StopReason::Admin(admin_msg)) => {
                admin_msg.reply("🛑 Stopped ONUW game.".yellow())?;
                if admin_msg.is_dm() {
                    lobby.say("🛑 ONUW game was stopped by an admin.".yellow())?;
                }
            }
            Outcome::Stopped(StopReason::Aborted(abort_msg)) => {
                if abort_msg.is_dm() {
                    abort_msg.reply("🛑 Aborted ONUW game.".yellow())?;
                }
                lobby.say(irc_format!(
                    "🛑 {} {}",
                    abort_msg.sender.nick().bold(),
                    "aborted the ONUW game — lobby is open!".yellow(),
                ))?;
            }
        }
    }

    Ok(())
}

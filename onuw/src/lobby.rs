//! Stateful three-phase lobby (`OnuwLobby`), standalone loop (`run_lobby_bot`), pre-game
//! recruitment (`recruit_players`), and `!roles` / `!marks` / `!artifacts` / `!scenarios` catalog
//! commands.

use ircgm::prelude::*;
use tokio::time::Instant;

use crate::artifacts::{find_artifact, Artifact};
use crate::day::is_active_in_room;
use crate::deck::{parse_role_spec_tokens, DeckBase, DeckSpec};
use crate::game::{run_dealt_game, GameConfig, MAX_PLAYERS, MIN_PLAYERS};
use crate::marks::{find_mark, Mark};
use crate::role::{GameSet, Role};
use crate::roles::{find_role, roles_in_game, ALL_ROLES};
use crate::scenarios::{
    all_scenarios, find_scenario, parse_player_count_filter, scenario_count, scenarios_in_pack,
    Difficulty, Scenario, ScenarioPack,
};
use crate::voting::GameReveal;

// ============================================================================
// Catalog & Help Formatters (`!help`, `!scenarios`, `!roles`, `!marks`, `!artifacts`)
// ============================================================================

pub fn send_lobby_help(lobby: &Room) -> Result<()> {
    let sep = " · ".grey();
    lobby.say(irc_format!(
        "🐺 {} — {}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        "One Night Ultimate Werewolf".bold().yellow(),
        "!start [pack|scenario]".bold().green(),
        sep.clone(),
        "!scenarios [pack|Np]".bold().cyan(),
        sep.clone(),
        "!scenario <name>".bold().cyan(),
        sep.clone(),
        "!roles [set]".bold().cyan(),
        sep.clone(),
        "!role <name>".bold().cyan(),
        sep.clone(),
        "!marks [name]".bold().cyan(),
        sep.clone(),
        "!artifacts [name]".bold().cyan(),
        sep,
        "!reveal".bold().yellow(),
    ))
}

pub fn send_open_lobby_help(reply_to: &ChatMessage, host: &User) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🌕 {} — {}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        "Open Lobby".bold().yellow(),
        "!join".bold().green(),
        sep.clone(),
        "!leave".bold(),
        sep.clone(),
        "!players".bold().cyan(),
        sep.clone(),
        "!deck".bold().cyan(),
        sep.clone(),
        "!scenarios [pack|Np]".bold().cyan(),
        sep.clone(),
        "!roles [set]".bold().cyan(),
        sep.clone(),
        "!marks".bold().cyan(),
        sep,
        "!artifacts".bold().cyan(),
    ))?;
    reply_to.reply(irc_format!(
        "  {} {}{}{}{}{}{}{}{}{}{}{}",
        format!("Host ({}):", host.nick()).bold(),
        "!preset <pack|scenario>".bold().cyan(),
        sep.clone(),
        "!add / !remove <role>".bold().cyan(),
        sep.clone(),
        "!clear".bold(),
        sep.clone(),
        "!reset".bold(),
        sep.clone(),
        "!go".bold().green(),
        sep,
        "!abort".bold().orange(),
    ))
}

pub fn send_active_game_help(reply_to: &ChatMessage) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🐺 {} — {}{}{}{}{}{}{}{}{}{}{}{}Day: {} / {}{}Vote: {}{}Cancel: {}",
        "Active Game".bold().yellow(),
        "!deck".bold().cyan(),
        sep.clone(),
        "!order".bold().cyan(),
        sep.clone(),
        "!players".bold().cyan(),
        sep.clone(),
        "!role <name>".bold().cyan(),
        sep.clone(),
        "!marks [name]".bold().cyan(),
        sep.clone(),
        "!artifacts [name]".bold().cyan(),
        sep.clone(),
        "!ready".bold(),
        "!time".bold(),
        sep.clone(),
        "!vote <nick>".bold().green(),
        sep,
        "!abort".bold().orange(),
    ))
}

fn format_pack_summary_bar() -> Text {
    let parts: Vec<Text> = ScenarioPack::ALL
        .iter()
        .map(|&pack| {
            irc_format!(
                "{} {}",
                pack.slug().bold().cyan(),
                format!("({})", scenarios_in_pack(pack).len()).grey(),
            )
        })
        .collect();
    Text::join(&parts, " · ".grey())
}

pub fn send_packs_info(room: &Room) -> Result<()> {
    let sep = " · ".grey();
    room.say(irc_format!(
        "📜 {} ({} total): {}",
        "Preset Scenarios".bold().cyan(),
        scenario_count(),
        format_pack_summary_bar(),
    ))?;
    room.say(irc_format!(
        "  Browse: {}{}{}{}{}",
        "!scenarios <pack>".bold().cyan(),
        sep.clone(),
        "!scenarios <3p..10p>".bold().cyan(),
        sep,
        "!scenario <name>".bold().cyan(),
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
        let sep = " · ".grey();
        reply_to.reply(irc_format!(
            "📜 {} ({} total): {}",
            "Preset Scenarios".bold().cyan(),
            scenario_count(),
            format_pack_summary_bar(),
        ))?;
        return reply_to.reply(irc_format!(
            "  Browse: {}{}{}{}{}",
            "!scenarios <pack>".bold().cyan(),
            sep.clone(),
            "!scenarios <3p..10p>".bold().cyan(),
            sep,
            "!scenario <name>".bold().cyan(),
        ));
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
                "Unknown pack or scenario '{q}'. Packs: base, daybreak, vampire, epic, alien, villains (or e.g. !scenarios 6p)."
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

    let sep = " · ".grey();
    if let Some(pack) = pack_filter {
        let title = match count_filter {
            Some(n) => format!("{} Scenarios for {n}p ({})", pack.label(), matched.len()),
            None => format!("{} Scenarios ({})", pack.label(), matched.len()),
        };
        reply_to.reply(irc_format!(
            "📜 {} — {} for cards{}{} or {} to play:",
            title.bold().cyan(),
            "!scenario <name>".bold(),
            sep,
            "!start <name>".bold().green(),
            "!preset <name>".bold().cyan(),
        ))?;

        for diff in Difficulty::ALL {
            let items: Vec<Text> = matched
                .iter()
                .filter(|s| s.difficulty == diff)
                .map(|s| {
                    irc_format!(
                        "{} {}",
                        s.slug.bold(),
                        format!("({})", s.range_label()).grey(),
                    )
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            for (chunk_idx, chunk) in items.chunks(5).enumerate() {
                if chunk_idx == 0 {
                    reply_to.reply(irc_format!(
                        "  {}: {}",
                        diff.styled_label(),
                        Text::join(chunk, " · ".grey()),
                    ))?;
                } else {
                    reply_to.reply(irc_format!(
                        "    {}",
                        Text::join(chunk, " · ".grey()),
                    ))?;
                }
            }
        }
        Ok(())
    } else {
        let n = count_filter.expect("either pack_filter or count_filter is Some");
        reply_to.reply(irc_format!(
            "📜 {} — {} for cards{}{} or {} to play:",
            format!("Scenarios for {n}p ({} total)", matched.len()).bold().cyan(),
            "!scenario <name>".bold(),
            sep,
            "!start <name>".bold().green(),
            "!preset <name>".bold().cyan(),
        ))?;

        for pack in ScenarioPack::ALL {
            let items: Vec<Text> = matched
                .iter()
                .filter(|s| s.pack == pack)
                .map(|s| {
                    irc_format!(
                        "{} {}",
                        s.slug.bold(),
                        format!("({})", s.difficulty.label()).grey(),
                    )
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            for (chunk_idx, chunk) in items.chunks(6).enumerate() {
                if chunk_idx == 0 {
                    reply_to.reply(irc_format!(
                        "  {}: {}",
                        pack.label().bold().cyan(),
                        Text::join(chunk, " · ".grey()),
                    ))?;
                } else {
                    reply_to.reply(irc_format!(
                        "    {}",
                        Text::join(chunk, " · ".grey()),
                    ))?;
                }
            }
        }
        Ok(())
    }
}

pub fn send_roles_catalog(reply_to: &ChatMessage, raw_filter: Option<&str>) -> Result<()> {
    let filter = raw_filter.map(str::trim).filter(|s| !s.is_empty());
    if let Some(q) = filter {
        if let Some(gs) = GameSet::parse(q) {
            let roles = roles_in_game(gs);
            let styled: Vec<Text> = roles.iter().map(|r| r.styled_name()).collect();
            return reply_to.reply(irc_format!(
                "🃏 {} ({} roles): {} {} {} for details",
                gs.label().bold(),
                styled.len(),
                Text::join(&styled, ", ".grey()),
                "—".grey(),
                "!role <name>".bold().cyan(),
            ));
        }
        if let Some(role) = find_role(q) {
            return send_single_role_info(reply_to, role);
        }
        return reply_to.reply(
            format_args!(
                "Unknown set or role '{q}'. Sets: base, daybreak, vampire, alien, villains."
            )
            .yellow(),
        );
    }

    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🃏 {} ({} total) — {} for details{}{}{}{}{}{}{}{}:",
        "Roles".bold().cyan(),
        ALL_ROLES.len(),
        "!role <name>".bold(),
        sep.clone(),
        "Evil".bold().red(),
        sep.clone(),
        "Vampire".bold().magenta(),
        sep.clone(),
        "Solo".bold().orange(),
        sep,
        "Village".bold().cyan(),
    ))?;
    for gs in GameSet::ALL {
        let roles = roles_in_game(gs);
        let styled: Vec<Text> = roles.iter().map(|r| r.styled_name()).collect();
        reply_to.reply(irc_format!(
            "  {} {}: {}",
            gs.label().bold(),
            format!("({})", styled.len()).grey(),
            Text::join(&styled, ", ".grey()),
        ))?;
    }
    Ok(())
}

pub fn send_single_role_info(reply_to: &ChatMessage, role: &'static dyn Role) -> Result<()> {
    let wake_str = match role.steps().last() {
        None => "Sleeps".to_owned(),
        Some(w) => crate::roles::step_wake_badge(w),
    };
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🃏 {}{}{}{}{}{}{} {} {}",
        role.styled_name(),
        sep.clone(),
        role.game().label(),
        sep.clone(),
        role.team().styled_label(),
        sep,
        wake_str.grey(),
        "—".grey(),
        role.info().summary,
    ))
}

pub fn send_marks_catalog(reply_to: &ChatMessage, raw_filter: Option<&str>) -> Result<()> {
    let filter = raw_filter.map(str::trim).filter(|s| !s.is_empty());
    if let Some(q) = filter {
        return match find_mark(q) {
            Some(mark) => send_single_mark_info(reply_to, mark),
            None => reply_to.reply(format_args!("Unknown mark '{q}' (try !marks).").yellow()),
        };
    }

    let styled: Vec<Text> = Mark::ALL.iter().copied().map(Mark::styled).collect();
    reply_to.reply(irc_format!(
        "🔖 {} ({}): {} {} {} for details",
        "Marks".bold().cyan(),
        styled.len(),
        Text::join(&styled, ", ".grey()),
        "—".grey(),
        "!mark <name>".bold().cyan(),
    ))
}

pub fn send_single_mark_info(reply_to: &ChatMessage, mark: Mark) -> Result<()> {
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🔖 {}{}{}{}{} {} {}",
        mark.styled(),
        sep.clone(),
        GameSet::Vampire.label(),
        sep,
        mark.source().grey(),
        "—".grey(),
        mark.summary(),
    ))
}

pub fn send_artifacts_catalog(reply_to: &ChatMessage, raw_filter: Option<&str>) -> Result<()> {
    let filter = raw_filter.map(str::trim).filter(|s| !s.is_empty());
    if let Some(q) = filter {
        return match find_artifact(q) {
            Some(art) => send_single_artifact_info(reply_to, art),
            None => {
                reply_to.reply(format_args!("Unknown artifact '{q}' (try !artifacts).").yellow())
            }
        };
    }

    let role_arts: Vec<Text> = Artifact::ALL
        .iter()
        .copied()
        .filter(|a| a.replaces_role_card())
        .map(Artifact::styled)
        .collect();
    let mod_arts: Vec<Text> = Artifact::ALL
        .iter()
        .copied()
        .filter(|a| !a.replaces_role_card())
        .map(Artifact::styled)
        .collect();

    reply_to.reply(irc_format!(
        "🏺 {} ({} total) — {} for details:",
        "Artifacts".bold().cyan(),
        Artifact::ALL.len(),
        "!artifact <name>".bold(),
    ))?;
    reply_to.reply(irc_format!(
        "  {} {}: {}",
        "Role-Replacing".bold(),
        format!("({})", role_arts.len()).grey(),
        Text::join(&role_arts, ", ".grey()),
    ))?;
    reply_to.reply(irc_format!(
        "  {} {}: {}",
        "Other Effects".bold(),
        format!("({})", mod_arts.len()).grey(),
        Text::join(&mod_arts, ", ".grey()),
    ))
}

pub fn send_single_artifact_info(reply_to: &ChatMessage, art: Artifact) -> Result<()> {
    let kind_str = if art.replaces_role_card() {
        "Replaces role"
    } else {
        "Keeps role"
    };
    let sep = " · ".grey();
    reply_to.reply(irc_format!(
        "🏺 {}{}{}{}{} {} {}",
        art.styled(),
        sep.clone(),
        "Daybreak (Curator)",
        sep,
        kind_str.grey(),
        "—".grey(),
        art.summary(),
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
                    msg.reply(format_args!("Unknown scenario '{q}' (try !scenarios).").yellow())?
                }
            }
        }
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_args("!roles") {
        send_roles_catalog(msg, Some(arg))?;
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_arg("!role") {
        match find_role(arg) {
            Some(r) => send_single_role_info(msg, r)?,
            None => msg.reply(format_args!("Unknown role '{arg}' (try !roles).").yellow())?,
        }
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_args("!marks").or_else(|| msg.cmd_args("!mark")) {
        send_marks_catalog(msg, Some(arg))?;
        return Ok(true);
    }
    if let Some(arg) = msg
        .cmd_args("!artifacts")
        .or_else(|| msg.cmd_args("!artifact"))
    {
        send_artifacts_catalog(msg, Some(arg))?;
        return Ok(true);
    }
    if let Some(arg) = msg.cmd_arg("!info") {
        if let Some(r) = find_role(arg) {
            send_single_role_info(msg, r)?;
        } else if let Some(m) = find_mark(arg) {
            send_single_mark_info(msg, m)?;
        } else if let Some(a) = find_artifact(arg) {
            send_single_artifact_info(msg, a)?;
        } else {
            msg.reply(
                format_args!(
                    "Unknown role, mark, or artifact '{arg}' (try !roles, !marks, !artifacts)."
                )
                .yellow(),
            )?;
        }
        return Ok(true);
    }
    Ok(false)
}

fn extract_launch_attempt_arg(msg: &ChatMessage) -> Option<&str> {
    msg.cmd_args("!start").map(str::trim)
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

    let sep = " · ".grey();
    let (init_min, init_max) = deck_spec.player_range();
    let range_str = format!("{}–{init_max}p", init_min.max(MIN_PLAYERS));
    let mode_desc: Text = match deck_spec.base {
        DeckBase::Scenario(scen) => irc_format!(
            "{} ({}{}{}{}{})",
            scen.name.bold().cyan(),
            scen.pack.label(),
            sep.clone(),
            scen.difficulty.styled_label(),
            sep.clone(),
            scen.range_label().grey(),
        ),
        DeckBase::Pack(pack) => irc_format!(
            "{} {}",
            pack.label().bold().cyan(),
            format!("({range_str})").grey(),
        ),
        DeckBase::Custom => irc_format!(
            "{} {}",
            "Custom Deck".bold().cyan(),
            format!("({range_str})").grey(),
        ),
    };
    room.say(irc_format!(
        "🌕 {} opened a {} lobby — {mode_desc}! Type {} to play ({}s).",
        host.nick().bold().yellow(),
        "One Night Ultimate Werewolf".bold(),
        "!join".bold().green(),
        config.lobby_duration.as_secs(),
    ))?;
    room.say(irc_format!(
        "  {} {}{}{}{}{}{}{}{}{}",
        "Host:".bold(),
        "!preset <pack|scenario>".bold().cyan(),
        sep.clone(),
        "!add / !remove <role>".bold().cyan(),
        sep.clone(),
        "!deck".bold().cyan(),
        sep.clone(),
        "!go".bold().green(),
        sep,
        "!abort".bold().orange(),
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

                    if msg.is_cmd("!join") || msg.is_cmd("!in") || msg.is_cmd("!play") {
                        let (_, cur_max) = deck_spec.player_range();
                        if players.len() >= cur_max {
                            msg.reply(
                                format_args!(
                                    "Lobby is full for {} (max {cur_max} players)!",
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
                        msg.reply(irc_format!(
                            "👥 {} {}",
                            format!("Lobby ({}/{cur_max}):", players.len()).bold(),
                            list.join(", "),
                        ))?;
                    } else if msg.is_cmd("!deck") {
                        msg.reply(irc_format!(
                            "🃏 {}",
                            deck_spec.preview_for_players(players.len()),
                        ))?;
                    } else if msg.is_cmd("!help") {
                        send_open_lobby_help(&msg, &players[0])?;
                    } else if msg.is_cmd("!reveal") {
                        msg.reply(
                            "⏳ Finish the current game first — !reveal is available after voting ends!"
                                .yellow(),
                        )?;
                    } else if handle_catalog_query(room, &msg)? {
                        continue;
                    } else if let Some(launch_arg) = extract_launch_attempt_arg(&msg) {
                        if is_host {
                            let preset_cmd =
                                if !launch_arg.is_empty() && DeckSpec::parse(launch_arg).is_some() {
                                    format!("!preset {launch_arg}")
                                } else {
                                    "!preset <pack|scenario>".to_owned()
                                };
                            msg.reply(irc_format!(
                                "Lobby is already open — use {} to change deck, {} to start now, or {} to cancel.",
                                preset_cmd.bold().cyan(),
                                "!go".bold().green(),
                                "!abort".bold().orange(),
                            ))?;
                        } else if players.contains(&msg.sender) {
                            msg.reply(irc_format!(
                                "Lobby is already open (hosted by {}) — waiting for more players or {}!",
                                players[0].nick().bold(),
                                "!go".bold().green(),
                            ))?;
                        } else {
                            msg.reply(irc_format!(
                                "Lobby is already open (hosted by {}) — type {} to join!",
                                players[0].nick().bold(),
                                "!join".bold().green(),
                            ))?;
                        }
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
                                "📜 Deck pack switched to {} — {}",
                                pack.label().bold().yellow(),
                                deck_spec.preview_for_players(players.len()),
                            ))?;
                        } else if let Some(scen) = find_scenario(q) {
                            if players.len() > scen.max_players() {
                                msg.reply(
                                    format_args!(
                                        "Scenario {} ({}) supports {} players, but {} are already in the lobby!",
                                        scen.name,
                                        scen.slug,
                                        scen.range_label(),
                                        players.len()
                                    )
                                    .yellow(),
                                )?;
                                continue;
                            }
                            deck_spec.set_scenario(scen);
                            room.say(irc_format!(
                                "📜 Scenario switched to {} {}",
                                format!("({}) —", scen.slug).cyan(),
                                deck_spec.preview_for_players(players.len()),
                            ))?;
                        } else {
                            msg.reply(
                                format_args!(
                                    "Unknown pack or scenario '{q}' — try !scenarios!"
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
                                        summary_parts.push(irc_format!(
                                            "+{added_n}x {}",
                                            role.styled_name()
                                        ));
                                    } else {
                                        summary_parts.push(irc_format!("+{}", role.styled_name()));
                                    }
                                }
                                room.say(irc_format!(
                                    "🃏 Added {} — {}",
                                    Text::join(&summary_parts, ", ".grey()),
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
                                        summary_parts.push(irc_format!(
                                            "-{rem_n}x {}",
                                            role.styled_name()
                                        ));
                                    } else {
                                        summary_parts.push(irc_format!("-{}", role.styled_name()));
                                    }
                                }
                                room.say(irc_format!(
                                    "🃏 Removed {} — {}",
                                    Text::join(&summary_parts, ", ".grey()),
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
                        room.say(irc_format!(
                            "🃏 Switched to pure {}! Add roles with {} (unfilled slots default to {}).",
                            "Custom Deck".bold().cyan(),
                            "!add <role>".bold().cyan(),
                            "Villager".bold().cyan(),
                        ))?;
                    } else if msg.is_cmd("!reset") {
                        if !is_host {
                            msg.reply("Only the lobby host can customize the deck!".yellow())?;
                            continue;
                        }
                        deck_spec.reset_overrides();
                        room.say(irc_format!(
                            "🃏 Reset custom overrides — {}",
                            deck_spec.preview_for_players(players.len()),
                        ))?;
                    } else if msg.is_cmd("!go") || msg.is_cmd("!force") {
                        if !is_host {
                            msg.reply(
                                irc_format!(
                                    "{} {}",
                                    "Only the lobby host can start early with".yellow(),
                                    "!go!".bold().yellow(),
                                ),
                            )?;
                            continue;
                        }
                        let (min_req, max_req) = deck_spec.player_range();
                        if players.len() < min_req.max(MIN_PLAYERS) {
                            msg.reply(
                                format_args!(
                                    "Need at least {} players for {} (currently {}).",
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
                                    "{} allows at most {max_req} players (currently {}).",
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
                    "Player count ({}) outside required {min_needed}–{max_req} range for {} — lobby closed.",
                    players.len(),
                    deck_spec.mode_label()
                )
                .yellow(),
            )?;
        }
        return Ok(None);
    }

    while deck_spec.added.len() > players.len() + 3 {
        deck_spec.added.pop();
    }

    Ok(Some((players, deck_spec)))
}

// ============================================================================
// Stateful Three-Phase Lobby (`OnuwLobby`) & Standalone Loop (`run_lobby_bot`)
// ============================================================================

/// A validated `!start [pack|scenario]` request ready to open pre-game recruitment.
#[derive(Debug)]
pub struct OnuwLaunch {
    pub starter: User,
    pub deck_spec: DeckSpec,
}

/// A recruited One Night Ultimate Werewolf round ready to be dealt and played.
#[derive(Debug)]
pub struct OnuwRound {
    pub host: User,
    pub players: Vec<User>,
    pub deck_spec: DeckSpec,
}

/// Stateful ONUW channel lobby exposing explicit idle (`wait_for_start`), recruitment
/// (`recruit`), and gameplay (`play_round`) phases so callers (such as a multi-game supervisor)
/// own the outer loop and admin cancellation boundaries.
#[derive(Debug)]
pub struct OnuwLobby {
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
                let host_gone = !is_active_in_room(&lobby, &host);
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
    tracing::warn!(error = %err, "ONUW game ended early");
    lobby.say(
        format_args!("⚠️ ONUW game ended early: {err} — lobby is open again!").yellow(),
    )?;
    Ok(())
}

fn handle_abort(lobby: &Room, abort_msg: &ChatMessage) -> Result<()> {
    if abort_msg.is_dm() {
        abort_msg.reply("🛑 Aborted ONUW game.".yellow())?;
    }
    lobby.say(irc_format!(
        "🛑 {} {}",
        abort_msg.sender.nick().bold(),
        "aborted the ONUW game — lobby is open!".yellow(),
    ))?;
    Ok(())
}

impl OnuwLobby {
    pub fn new(config: GameConfig) -> Self {
        Self {
            config,
            pending_reveal: None,
            announced_help: false,
        }
    }

    /// Phase 1: handles idle lobby commands (`!help`, `!reveal`, catalog lookups) until a player
    /// starts a round with `!start [pack|scenario]`.
    pub async fn wait_for_start(&mut self, lobby: &Room) -> Result<OnuwLaunch> {
        if !self.announced_help {
            send_lobby_help(lobby)?;
            self.announced_help = true;
        }

        let mut inbox = lobby.inbox().lossy();
        loop {
            let idle_msg = inbox.next_chat().await?;
            if idle_msg.is_cmd("!help") {
                send_lobby_help(lobby)?;
                continue;
            }
            if idle_msg.is_cmd("!reveal") {
                match self.pending_reveal.take() {
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

            let Some(raw_args) = idle_msg.cmd_args("!start") else {
                continue;
            };
            let trimmed = raw_args.trim();
            let Some(deck_spec) = DeckSpec::parse(trimmed) else {
                idle_msg.reply(
                    format_args!("Unknown pack or scenario '{trimmed}' — try !scenarios!").yellow(),
                )?;
                continue;
            };

            return Ok(OnuwLaunch {
                starter: idle_msg.sender.clone(),
                deck_spec,
            });
        }
    }

    /// Phase 2: runs open pre-game recruitment (`!join`/`!preset`/`!add`/`!go`, host `!abort`).
    /// Returns `Ok(Some(round))` when ready to deal, or `Ok(None)` if cancelled/emptied/timed out
    /// without enough players.
    pub async fn recruit(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        launch: OnuwLaunch,
    ) -> Result<Option<OnuwRound>> {
        let outcome = recruit_players(lobby, launch.starter.clone(), launch.deck_spec, &self.config)
            .until(abort_stop_listener(bot, lobby, &launch.starter))
            .await;

        match outcome {
            Outcome::Completed(Ok(Some((players, deck_spec)))) => {
                let host = players[0].clone();
                Ok(Some(OnuwRound {
                    host,
                    players,
                    deck_spec,
                }))
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

    /// Phase 3: deals roles and runs a full Dusk/Night/Day/Voting round to completion (or host
    /// `!abort`). Stores the post-game [`GameReveal`] for the next `!reveal`.
    pub async fn play_round(
        &mut self,
        bot: &Bot,
        lobby: &Room,
        round: OnuwRound,
    ) -> Result<()> {
        self.pending_reveal = None;

        let (roster, initial_board) = match round.deck_spec.deal(&round.players) {
            Ok(dealt) => dealt,
            Err(_) => {
                return handle_round_error(lobby, Error::InvalidInput("invalid ONUW deck size"));
            }
        };

        let host_for_busy = round.host.clone();
        let lobby_for_busy = lobby.clone();
        let busy_responder = lobby.inbox().on_map(move |msg| {
            if extract_launch_attempt_arg(msg).is_some() {
                let abort_hint = if is_active_in_room(&lobby_for_busy, &host_for_busy) {
                    format!("{} can type !abort", host_for_busy.nick())
                } else {
                    "type !abort".to_owned()
                };
                let _ = lobby_for_busy.say(
                    format_args!("⏳ An ONUW game is already in progress ({abort_hint} to cancel)!")
                        .yellow(),
                );
            } else if msg.is_cmd("!reveal") {
                let _ = msg.reply(
                    "⏳ Finish the current game first — !reveal is available after voting ends!"
                        .yellow(),
                );
            }
            None::<ChatMessage>
        });

        let outcome = run_dealt_game(
            bot,
            lobby,
            &roster,
            initial_board,
            &round.deck_spec.mode_label(),
            &self.config,
        )
        .until(abort_stop_listener(bot, lobby, &round.host))
        .or_until(busy_responder)
        .await;

        match outcome {
            Outcome::Completed(Ok(summary)) => {
                self.pending_reveal = Some(summary.reveal);
                lobby.say(irc_format!(
                    "🏁 {} Discuss what happened, then type {} for the full reveal — or {} for another round!",
                    "Round complete!".bold().green(),
                    "!reveal".bold().yellow(),
                    "!start [pack|scenario]".bold().cyan(),
                ))?;
            }
            Outcome::Completed(Err(err)) => handle_round_error(lobby, err)?,
            Outcome::Stopped(abort_msg) => handle_abort(lobby, &abort_msg)?,
        }

        Ok(())
    }
}

pub async fn run_lobby_bot(bot: &Bot, lobby: &Room, config: &GameConfig) -> Result<()> {
    let mut state = OnuwLobby::new(config.clone());
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

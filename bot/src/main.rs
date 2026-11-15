//! Standalone CLI binary for `ircgm-games` (`bot`).
//!
//! ```sh
//! cargo run -p bot -- --server irc.libera.chat:6697 --lobby '#onuw' --nick ONUWBot
//! ```

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::time::Duration;

use ircgm::irc::client::data::Config as IrcConfig;
use ircgm::prelude::*;
use ircgm::{BotConfig, DisconnectReason};
use onuw::lobby::is_admin_msg;
use onuw::{run_lobby_bot, GameConfig, ScenarioPack, ALL_ROLES};

const DEFAULT_SERVER: &str = "127.0.0.1:6667";
const DEFAULT_LOBBY: &str = "#onuw";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BotCli {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) use_tls: bool,
    pub(crate) lobby: String,
    pub(crate) lobby_key: Option<String>,
    pub(crate) nick: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) admins: Vec<String>,
    pub(crate) fast: bool,
    pub(crate) no_dm_check: bool,
    pub(crate) reconnect: bool,
    pub(crate) verbose: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParseOutcome {
    Run(BotCli),
    Help(String),
}

fn usage_text(bin: &str) -> String {
    let packs = ScenarioPack::ALL
        .iter()
        .map(|p| format!("  {:<16} {}", p.slug(), p.label()))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\
Usage: {bin} [OPTIONS]

One Night Ultimate Werewolf IRC bot ({role_count} roles across Base + Daybreak, Vampire, Alien, Super Villains).

Options:
  -s, --server <HOST[:PORT]>   IRC server address (default: {DEFAULT_SERVER}; supports irc:// and ircs://)
  -p, --port <PORT>            Override IRC server port (default: 6697 with TLS, 6667 without TLS)
  -l, --lobby <CHANNEL>        Lobby channel to join (default: {DEFAULT_LOBBY})
      --lobby-key <KEY>        Optional channel key (+k) for joining --lobby
  -n, --nick <NICK>            Bot nickname (default: onuw-<random4>)
      --password <PASS>        Optional server PASS (on Libera.Chat: <account>:<password> for NickServ)
      --tls / --no-tls         Force TLS on or off (auto-enabled for ircs:// and ports 6697/7000/7070)
      --admin <ACCOUNT>        Services account allowed to force `!stop` / `!quit` (repeatable)
      --fast                   Shorter timers for rapid speed rounds
      --no-dm-check            Skip pre-game DM `ok` reachability check
      --reconnect              Automatically reconnect after unexpected disconnects
  -v, --verbose                Enable debug-level tracing output
  -h, --help                   Show this help message

Scenario packs:
{packs}

Lobby commands:
  !onuw [pack|scenario]        Open an ONUW lobby (host can customize with !preset, !add, !remove, !clear, !reset, !deck, !go)
  !scenarios [pack|Np]         Browse preset scenarios by expansion pack or player count
  !roles [game]                List roles (all or by game: base, daybreak, vampire, alien, villains)
  !role <name>                 Inspect a role's team, game, wake order, and ability
  !abort                       Cancel the active game",
        role_count = ALL_ROLES.len(),
    )
}

fn parse_server_spec(
    spec: &str,
) -> std::result::Result<(String, Option<u16>, Option<bool>), String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err("--server cannot be empty".into());
    }

    let (scheme_tls, rest) = if let Some(r) = trimmed.strip_prefix("ircs://") {
        (Some(true), r)
    } else if let Some(r) = trimmed.strip_prefix("irc://") {
        (Some(false), r)
    } else {
        (None, trimmed)
    };
    let rest = rest.trim_end_matches('/');
    if rest.is_empty() {
        return Err(format!("invalid --server `{spec}`: missing host"));
    }

    if let Some(after_open) = rest.strip_prefix('[') {
        let Some((ipv6_host, tail)) = after_open.split_once(']') else {
            return Err(format!("invalid IPv6 address in --server `{spec}`"));
        };
        if ipv6_host.is_empty() {
            return Err(format!("empty IPv6 host in --server `{spec}`"));
        }
        let port = if tail.is_empty() {
            None
        } else if let Some(port_str) = tail.strip_prefix(':') {
            let p = port_str
                .parse::<u16>()
                .map_err(|_| format!("invalid port `{port_str}` in --server `{spec}`"))?;
            Some(p)
        } else {
            return Err(format!("unexpected trailing `{tail}` in --server `{spec}`"));
        };
        return Ok((ipv6_host.to_owned(), port, scheme_tls));
    }

    if let Some((host_part, port_part)) = rest.rsplit_once(':') {
        if !host_part.contains(':') {
            if host_part.is_empty() {
                return Err(format!("missing host before `:` in --server `{spec}`"));
            }
            let p = port_part
                .parse::<u16>()
                .map_err(|_| format!("invalid port `{port_part}` in --server `{spec}`"))?;
            return Ok((host_part.to_owned(), Some(p), scheme_tls));
        }
    }

    Ok((rest.to_owned(), None, scheme_tls))
}

fn normalize_channel(raw: &str) -> std::result::Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("--lobby channel cannot be empty".into());
    }
    if trimmed.chars().any(char::is_whitespace) || trimmed.contains(',') {
        return Err(format!(
            "invalid --lobby `{trimmed}`: must be a single channel name without spaces or commas"
        ));
    }
    if trimmed.starts_with(['#', '&', '+', '!']) {
        Ok(trimmed.to_owned())
    } else {
        Ok(format!("#{trimmed}"))
    }
}

pub(crate) fn parse_cli<I, S>(args: I) -> std::result::Result<ParseOutcome, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut iter = args.into_iter().map(Into::into);
    let bin = iter.next().unwrap_or_else(|| "bot".to_owned());

    let mut raw_server: Option<String> = None;
    let mut port_override: Option<u16> = None;
    let mut tls_override: Option<bool> = None;
    let mut lobby: Option<String> = None;
    let mut lobby_key: Option<String> = None;
    let mut nick: Option<String> = None;
    let mut password: Option<String> = None;
    let mut admins: Vec<String> = Vec::new();
    let mut fast = false;
    let mut no_dm_check = false;
    let mut reconnect = false;
    let mut verbose = false;

    let next_val =
        |flag: &str, it: &mut dyn Iterator<Item = String>| -> std::result::Result<String, String> {
            it.next()
                .ok_or_else(|| format!("missing value for `{flag}`"))
        };

    while let Some(arg) = iter.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(ParseOutcome::Help(usage_text(&bin)));
        } else if arg == "-v" || arg == "--verbose" {
            verbose = true;
        } else if arg == "--fast" {
            fast = true;
        } else if arg == "--no-dm-check" {
            no_dm_check = true;
        } else if arg == "--reconnect" {
            reconnect = true;
        } else if arg == "--tls" {
            tls_override = Some(true);
        } else if arg == "--no-tls" {
            tls_override = Some(false);
        } else if arg == "-s" || arg == "--server" {
            raw_server = Some(next_val(&arg, &mut iter)?);
        } else if let Some(v) = arg.strip_prefix("--server=") {
            raw_server = Some(v.to_owned());
        } else if arg == "-p" || arg == "--port" {
            let v = next_val(&arg, &mut iter)?;
            let p = v
                .parse::<u16>()
                .map_err(|_| format!("invalid port `{v}` for `{arg}`"))?;
            port_override = Some(p);
        } else if let Some(v) = arg.strip_prefix("--port=") {
            let p = v
                .parse::<u16>()
                .map_err(|_| format!("invalid port `{v}` for `--port`"))?;
            port_override = Some(p);
        } else if arg == "-l" || arg == "--lobby" || arg == "--channel" {
            lobby = Some(next_val(&arg, &mut iter)?);
        } else if let Some(v) = arg
            .strip_prefix("--lobby=")
            .or_else(|| arg.strip_prefix("--channel="))
        {
            lobby = Some(v.to_owned());
        } else if arg == "--lobby-key" {
            lobby_key = Some(next_val(&arg, &mut iter)?);
        } else if let Some(v) = arg.strip_prefix("--lobby-key=") {
            lobby_key = Some(v.to_owned());
        } else if arg == "-n" || arg == "--nick" {
            let v = next_val(&arg, &mut iter)?;
            if v.trim().is_empty() {
                return Err("--nick cannot be empty".into());
            }
            nick = Some(v.trim().to_owned());
        } else if let Some(v) = arg.strip_prefix("--nick=") {
            if v.trim().is_empty() {
                return Err("--nick cannot be empty".into());
            }
            nick = Some(v.trim().to_owned());
        } else if arg == "--password" || arg == "--pass" {
            password = Some(next_val(&arg, &mut iter)?);
        } else if let Some(v) = arg
            .strip_prefix("--password=")
            .or_else(|| arg.strip_prefix("--pass="))
        {
            password = Some(v.to_owned());
        } else if arg == "--admin" {
            let v = next_val(&arg, &mut iter)?;
            if !v.trim().is_empty() {
                admins.push(v.trim().to_owned());
            }
        } else if let Some(v) = arg.strip_prefix("--admin=") {
            if !v.trim().is_empty() {
                admins.push(v.trim().to_owned());
            }
        } else {
            return Err(format!("unknown argument `{arg}` (pass --help for usage)"));
        }
    }

    let server_str = raw_server.as_deref().unwrap_or(DEFAULT_SERVER);
    let (host, parsed_port, scheme_tls) = parse_server_spec(server_str)?;

    let is_tls_port = |p: u16| matches!(p, 6697 | 7000 | 7070);
    let port = match (port_override, parsed_port) {
        (Some(p), _) => p,
        (None, Some(p)) => p,
        (None, None) => {
            if tls_override.or(scheme_tls).unwrap_or(false) {
                6697
            } else {
                6667
            }
        }
    };
    let use_tls = tls_override
        .or(scheme_tls)
        .unwrap_or_else(|| is_tls_port(port));

    let lobby_chan = normalize_channel(lobby.as_deref().unwrap_or(DEFAULT_LOBBY))?;

    Ok(ParseOutcome::Run(BotCli {
        host,
        port,
        use_tls,
        lobby: lobby_chan,
        lobby_key: lobby_key.filter(|k| !k.is_empty()),
        nick,
        password: password.filter(|p| !p.is_empty()),
        admins,
        fast,
        no_dm_check,
        reconnect,
        verbose,
    }))
}

fn random_suffix_4() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(4);
    for _ in 0..4 {
        out.push(ALPHABET[fastrand::usize(..ALPHABET.len())] as char);
    }
    out
}

async fn admin_quit_loop(bot: &Bot, lobby: &Room, admins: &[String]) -> Result<bool> {
    if admins.is_empty() {
        bot.shutdown_requested().await;
        return Ok(false);
    }

    let mut inbox = bot.inbox().lossy();
    while let Outcome::Completed(msg) = inbox.wait_cmd("!quit").until(bot.shutdown_requested()).await
    {
        let msg = msg?;
        if (msg.is_dm() || msg.is_in(lobby)) && is_admin_msg(&msg, admins) {
            msg.reply("👋 Shutting down ONUW bot!".yellow())?;
            bot.quit(Some("Requested by admin")).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

async fn run_session(cli: &BotCli, game_cfg: &GameConfig) -> Result<bool> {
    let primary_nick = cli
        .nick
        .clone()
        .unwrap_or_else(|| format!("onuw-{}", random_suffix_4()));
    let alt_nicks = vec![
        format!("{primary_nick}_"),
        format!("{primary_nick}__"),
        format!("onuw-{}", random_suffix_4()),
    ];

    let irc_cfg = IrcConfig {
        nickname: Some(primary_nick.clone()),
        alt_nicks,
        username: Some("onuw".into()),
        realname: Some("One Night Ultimate Werewolf bot (ircgm)".into()),
        server: Some(cli.host.clone()),
        port: Some(cli.port),
        password: cli.password.clone(),
        use_tls: Some(cli.use_tls),
        ..Default::default()
    };

    let bot_cfg = BotConfig::new(irc_cfg).with_command_prefixes(["!"]);

    tracing::info!(
        server = %format!("{}:{}", cli.host, cli.port),
        tls = cli.use_tls,
        nick = %primary_nick,
        lobby = %cli.lobby,
        "connecting to IRC server"
    );

    let bot = Bot::connect_with(bot_cfg).await?;
    tracing::info!(nick = %bot.nick(), "registered with IRC server");

    let lobby = match &cli.lobby_key {
        Some(key) => bot.join_with_key(&cli.lobby, key).await?,
        None => bot.join(&cli.lobby).await?,
    };
    tracing::info!(lobby = %lobby.name(), "joined lobby channel");

    let (_, admin_quit) = tokio::try_join!(
        run_lobby_bot(&bot, &lobby, &cli.admins, game_cfg),
        admin_quit_loop(&bot, &lobby, &cli.admins),
    )?;
    Ok(admin_quit)
}

async fn run_live(cli: BotCli) -> Result<()> {
    let mut game_cfg = if cli.fast {
        GameConfig::fast_mode()
    } else {
        GameConfig::default()
    };
    if cli.no_dm_check {
        game_cfg.dm_check_timeout = None;
    }

    loop {
        match run_session(&cli, &game_cfg).await {
            Ok(_)
            | Err(Error::Disconnected(DisconnectReason::Quit | DisconnectReason::BotDropped)) => {
                return Ok(());
            }
            Err(err) if cli.reconnect => {
                tracing::warn!(error = %err, "session disconnected; reconnecting in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = match parse_cli(std::env::args()) {
        Ok(ParseOutcome::Help(help)) => {
            println!("{help}");
            return ExitCode::SUCCESS;
        }
        Ok(ParseOutcome::Run(cli)) => cli,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    let max_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt()
        .with_max_level(max_level)
        .with_target(false)
        .init();

    if let Err(err) = run_live(cli).await {
        tracing::error!(error = %err, "bot exited with error");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod test {
    use super::{parse_cli, ParseOutcome};

    #[test]
    fn parse_cli_supports_libera_tls_fast_and_admin_flags() {
        let ParseOutcome::Run(cli) = parse_cli([
            "bot",
            "--server=ircs://irc.libera.chat",
            "--lobby=onuw",
            "--nick",
            "ONUW",
            "--admin",
            "seirl",
            "--fast",
            "--no-dm-check",
            "--reconnect",
        ])
        .unwrap()
        else {
            panic!("expected Run");
        };
        assert_eq!(cli.host, "irc.libera.chat");
        assert_eq!(cli.port, 6697);
        assert!(cli.use_tls);
        assert_eq!(cli.lobby, "#onuw");
        assert_eq!(cli.nick.as_deref(), Some("ONUW"));
        assert_eq!(cli.admins, vec!["seirl".to_owned()]);
        assert!(cli.fast);
        assert!(cli.no_dm_check);
        assert!(cli.reconnect);
    }
}

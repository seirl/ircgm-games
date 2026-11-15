//! Command-line argument parsing for `ircgm-games-bot`.

use crate::games::{usage_games_section, EnabledGames};

pub const DEFAULT_SERVER: &str = "127.0.0.1:6667";
pub const DEFAULT_LOBBY: &str = "#ircgm";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotCli {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
    pub lobby: String,
    pub lobby_key: Option<String>,
    pub games: EnabledGames,
    pub nick: Option<String>,
    pub password: Option<String>,
    pub admins: Vec<String>,
    pub verbose: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseOutcome {
    Run(BotCli),
    Help(String),
}

fn usage_text(bin: &str) -> String {
    let available = usage_games_section();
    format!(
        "\
Usage: {bin} [OPTIONS]

IRC game master bot for ircgm-games.

Options:
  -s, --server <HOST[:PORT]>   IRC server address (default: {DEFAULT_SERVER}; supports irc:// and ircs://)
  -p, --port <PORT>            Override IRC server port (default: 6697 with TLS, 6667 without TLS)
  -l, --lobby <CHANNEL>        Lobby channel to join (default: {DEFAULT_LOBBY})
      --lobby-key <KEY>        Optional channel key (+k) for joining --lobby
  -g, --games <LIST>           Comma-separated / repeatable games to enable (default: all)
  -n, --nick <NICK>            Bot nickname (default: ircgm-<random4>)
      --password <PASS>        Optional server PASS (on Libera.Chat: <account>:<password> for NickServ)
      --tls / --no-tls         Force TLS on or off (auto-enabled for ircs:// and ports 6697/7000/7070)
      --admin <ACCOUNT>        Services account allowed to run `!fast`, `!stop`, and `!quit` (repeatable)
  -v, --verbose                Enable debug-level tracing output
  -h, --help                   Show this help message

Available games:
{available}

Lobby commands:
  !play <game> [args]          Start an enabled game (`!<game>` shorthand also works)
  !random                      Start a random enabled game
  !reveal                      Reveal hidden roles and action log from the last finished game
  !games / !help               List enabled games and commands
  !abort                       Cancel the active game
  !fast [on|off]               (Admin) Toggle fast-timer mode for upcoming games
  !stop / !quit                (Admin) Stop the active game / shut down the bot"
    )
}

pub(crate) fn parse_server_spec(
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

pub(crate) fn normalize_channel(raw: &str) -> std::result::Result<String, String> {
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

pub fn parse_cli<I, S>(args: I) -> std::result::Result<ParseOutcome, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut iter = args.into_iter().map(Into::into);
    let bin = iter.next().unwrap_or_else(|| "ircgm-games-bot".to_owned());

    let mut raw_server: Option<String> = None;
    let mut port_override: Option<u16> = None;
    let mut tls_override: Option<bool> = None;
    let mut lobby: Option<String> = None;
    let mut lobby_key: Option<String> = None;
    let mut game_specs: Vec<String> = Vec::new();
    let mut nick: Option<String> = None;
    let mut password: Option<String> = None;
    let mut admins: Vec<String> = Vec::new();
    let mut verbose = false;

    let next_val =
        |flag: &str, it: &mut dyn Iterator<Item = String>| -> std::result::Result<String, String> {
            it.next()
                .ok_or_else(|| format!("missing value for `{flag}`"))
        };

    let match_opt = |arg: &str,
                     short: Option<&str>,
                     longs: &[&str],
                     it: &mut dyn Iterator<Item = String>|
     -> std::result::Result<Option<String>, String> {
        if short == Some(arg) || longs.contains(&arg) {
            return next_val(arg, it).map(Some);
        }
        for long in longs {
            if let Some(rest) = arg.strip_prefix(long).and_then(|s| s.strip_prefix('=')) {
                return Ok(Some(rest.to_owned()));
            }
        }
        Ok(None)
    };

    while let Some(arg) = iter.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(ParseOutcome::Help(usage_text(&bin)));
        } else if arg == "-v" || arg == "--verbose" {
            verbose = true;
        } else if arg == "--tls" {
            tls_override = Some(true);
        } else if arg == "--no-tls" {
            tls_override = Some(false);
        } else if let Some(v) = match_opt(&arg, Some("-s"), &["--server"], &mut iter)? {
            raw_server = Some(v);
        } else if let Some(v) = match_opt(&arg, Some("-p"), &["--port"], &mut iter)? {
            let p = v
                .parse::<u16>()
                .map_err(|_| format!("invalid port `{v}` for `--port`"))?;
            port_override = Some(p);
        } else if let Some(v) = match_opt(&arg, Some("-l"), &["--lobby", "--channel"], &mut iter)? {
            lobby = Some(v);
        } else if let Some(v) = match_opt(&arg, None, &["--lobby-key"], &mut iter)? {
            lobby_key = Some(v);
        } else if let Some(v) = match_opt(&arg, Some("-g"), &["--games", "--game"], &mut iter)? {
            game_specs.push(v);
        } else if let Some(v) = match_opt(&arg, Some("-n"), &["--nick"], &mut iter)? {
            if v.trim().is_empty() {
                return Err("--nick cannot be empty".into());
            }
            nick = Some(v.trim().to_owned());
        } else if let Some(v) = match_opt(&arg, None, &["--password", "--pass"], &mut iter)? {
            password = Some(v);
        } else if let Some(v) = match_opt(&arg, None, &["--admin"], &mut iter)? {
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
    let games = EnabledGames::from_specs(&game_specs)?;

    Ok(ParseOutcome::Run(BotCli {
        host,
        port,
        use_tls,
        lobby: lobby_chan,
        lobby_key: lobby_key.filter(|k| !k.is_empty()),
        games,
        nick,
        password: password.filter(|p| !p.is_empty()),
        admins,
        verbose,
    }))
}

#[cfg(test)]
#[path = "cli_test.rs"]
mod test;

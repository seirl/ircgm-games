//! Standalone CLI binary for `ircgm-games` (`ircgm-games-bot`).
//!
//! ```sh
//! cargo run -p ircgm-games-bot -- --server ircs://irc.libera.chat --lobby '#ircgm' --games onuw
//! ```

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::time::Duration;

use ircgm::irc::client::data::Config as IrcConfig;
use ircgm::prelude::*;
use ircgm::{BotConfig, DisconnectReason};
use ircgm_games_bot::{admin_quit_loop, parse_cli, run_lobby, BotCli, ParseOutcome};

fn random_suffix_4() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(4);
    for _ in 0..4 {
        out.push(ALPHABET[fastrand::usize(..ALPHABET.len())] as char);
    }
    out
}

async fn run_session(cli: &BotCli) -> Result<bool> {
    let primary_nick = cli
        .nick
        .clone()
        .unwrap_or_else(|| format!("ircgm-{}", random_suffix_4()));
    let alt_nicks = vec![
        format!("{primary_nick}_"),
        format!("{primary_nick}__"),
        format!("ircgm-{}", random_suffix_4()),
    ];

    let irc_cfg = IrcConfig {
        nickname: Some(primary_nick.clone()),
        alt_nicks,
        username: Some("ircgm".into()),
        realname: Some("ircgm-games bot".into()),
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
        games = %cli.games.slug_list(),
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
        run_lobby(&bot, &lobby, &cli.admins, &cli.games),
        admin_quit_loop(&bot, &lobby, &cli.admins),
    )?;
    Ok(admin_quit)
}

async fn run_live(cli: BotCli) -> Result<()> {
    loop {
        match run_session(&cli).await {
            Ok(_)
            | Err(Error::Disconnected(DisconnectReason::Quit | DisconnectReason::BotDropped)) => {
                return Ok(());
            }
            Err(err) => {
                tracing::warn!(error = %err, "session disconnected; reconnecting in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
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

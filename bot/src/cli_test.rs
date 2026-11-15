use super::{normalize_channel, parse_cli, parse_server_spec, ParseOutcome};
use crate::games::EnabledGames;

#[test]
fn parse_cli_supports_libera_tls_lobby_games_and_admin_flags() {
    let ParseOutcome::Run(cli) = parse_cli([
        "ircgm-games-bot",
        "--server=ircs://irc.libera.chat",
        "--lobby=onuw",
        "--games=onuw",
        "--nick",
        "ONUW",
        "--admin",
        "seirl",
    ])
    .unwrap()
    else {
        panic!("expected Run");
    };
    assert_eq!(cli.host, "irc.libera.chat");
    assert_eq!(cli.port, 6697);
    assert!(cli.use_tls);
    assert_eq!(cli.lobby, "#onuw");
    assert_eq!(cli.games, EnabledGames::from_specs(&["onuw"]).unwrap());
    assert_eq!(cli.nick.as_deref(), Some("ONUW"));
    assert_eq!(cli.admins, vec!["seirl".to_owned()]);
}

#[test]
fn parse_cli_defaults_to_all_games_and_rejects_unknown_game_or_removed_flags() {
    let ParseOutcome::Run(cli) = parse_cli(["ircgm-games-bot"]).unwrap() else {
        panic!("expected Run");
    };
    assert_eq!(cli.games, EnabledGames::all());
    assert_eq!(cli.lobby, "#ircgm");

    let err = parse_cli(["ircgm-games-bot", "--games", "onuw,nope"]).unwrap_err();
    assert!(err.contains("unknown game `nope`"));

    let fast_err = parse_cli(["ircgm-games-bot", "--fast"]).unwrap_err();
    assert!(fast_err.contains("unknown argument `--fast`"));
}

#[test]
fn server_and_channel_normalization_handles_ipv6_and_prefixes() {
    assert_eq!(
        parse_server_spec("ircs://[::1]:6697").unwrap(),
        ("::1".into(), Some(6697), Some(true))
    );
    assert_eq!(normalize_channel("games").unwrap(), "#games");
    assert_eq!(normalize_channel("##games").unwrap(), "##games");
    assert!(normalize_channel("#a,#b").is_err());
}

use std::time::Duration;

use irc::proto::Command;
use ircgm::testing::test_harness;

use crate::deck::DeckSpec;
use crate::game::GameConfig;
use crate::lobby::{recruit_players, run_lobby_bot};

#[tokio::test(start_paused = true)]
async fn full_lobby_custom_deck_to_voting_chronicle_and_abort_cleanup() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#onuw", &["Alice", "Bob", "Carol"]);
    // Set an initial topic so we can verify RAII topic restoration!
    srv.send_line(":Alice!~Alice@test.host TOPIC #onuw :Welcome to #onuw!\r\n");
    srv.settle().await;

    let lobby = bot.room("#onuw").unwrap();
    let cfg = GameConfig::for_tests();
    let bot_task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        async move { run_lobby_bot(&bot, &lobby, &["adminacct".into()], &cfg).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#onuw");

    // 1. Start a game, verify open-lobby `!help` and host `!onuw vampire` hint, then customize deck:
    srv.chat("Alice", "#onuw", "!onuw");
    srv.settle().await;
    let _ = srv.take_messages_to("#onuw");
    srv.chat("Bob", "#onuw", "!help");
    srv.chat("Alice", "#onuw", "!onuw vampire");
    srv.settle().await;
    let open_lobby_out = srv.take_messages_to("#onuw").join("\n");
    assert!(open_lobby_out.contains("Open Lobby"));
    assert!(open_lobby_out.contains("!preset vampire"));
    assert!(!open_lobby_out.contains("already in progress"));
    assert!(
        !open_lobby_out.contains('`'),
        "expected zero backticks in open lobby output, got:\n{open_lobby_out}"
    );

    srv.chat("Bob", "#onuw", "!join");
    srv.chat("Carol", "#onuw", "!join");
    srv.chat("Alice", "#onuw", "!clear");
    // 6 identical Werewolves? Wait: let's add 6x Insomniac or 6x Villager so Night finishes with 0 prompts!
    srv.chat("Alice", "#onuw", "!add 6x insomniac");
    srv.chat("Alice", "#onuw", "!deck");
    srv.chat("Alice", "#onuw", "!go");
    // Fixed-clock anti-timing-attack tier for Insomniac runs for `tier_budget` (5s in test config):
    srv.advance(Duration::from_secs(5)).await;

    // Night ran with 3 Insomniacs -> Day started! Query `!players` in DM and `!ready` in channel:
    srv.dm("Bob", "!players");
    srv.settle().await;
    let bob_dms = srv.take_messages_to("Bob").join("\n");
    assert!(bob_dms.contains("Your initial card is: Insomniac"));
    assert!(bob_dms.contains("checked your card") || bob_dms.contains("check your card"));
    assert!(bob_dms.contains("Table circle: Alice ↔ Bob ↔ Carol (wraps)"));

    srv.chat("Alice", "#onuw", "!ready");
    srv.chat("Bob", "#onuw", "!ready");
    srv.chat("Carol", "#onuw", "!ready");
    srv.settle().await;

    // Voting phase is public in `#onuw` (`!vote <nick>`): numeric seat ID `"1"` is rejected,
    // and nick-based circle vote 1-1-1 -> 0 deaths -> Village wins!
    srv.chat("Alice", "#onuw", "!vote Bob");
    srv.chat("Bob", "#onuw", "!vote Carol");
    srv.chat("Carol", "#onuw", "!vote 1");
    srv.settle().await;
    let mid_chan = srv.take_messages_to("#onuw").join("\n");
    assert!(mid_chan.contains("Unknown player nickname."));
    srv.chat("Carol", "#onuw", "!vote Alice");
    srv.settle().await;

    let chan_out = srv.take_messages_to("#onuw").join("\n");
    assert!(chan_out.contains("🗳️ VOTING — Votes: Alice (1), Bob (1), Carol (1) · Nobody died!"));
    assert!(chan_out.contains("🏆 Winners: Alice (Village), Bob (Village), Carol (Village)"));
    assert!(chan_out.contains("Round complete!"));
    assert!(chan_out.contains("!reveal"));
    assert!(!chan_out.contains('`'));
    // Full game record (center cards, roles, and chronicle) must NOT be spoiled before `!reveal`:
    assert!(!chan_out.contains("🗳️ VOTING REVEAL"));
    assert!(!chan_out.contains("📜 What Happened:"));
    assert!(!chan_out.contains("Insomniac"));
    assert!(!chan_out.contains("🃏 Roles:"));

    // `!reveal` sends center cards, compact roles summary, and chronicle once:
    srv.chat("Bob", "#onuw", "!reveal");
    srv.settle().await;
    let reveal_out = srv.take_messages_to("#onuw").join("\n");
    assert!(reveal_out.contains("🗳️ VOTING REVEAL — Center:"));
    assert!(reveal_out.contains("🃏 Roles: Alice: Insomniac · Bob: Insomniac · Carol: Insomniac"));
    assert!(reveal_out.contains("📜 What Happened:"));
    assert!(reveal_out.contains("Insomniac (Alice) checked their card and saw Insomniac."));
    assert!(!reveal_out.contains('`'));

    // Repeating `!reveal` does not re-spam the log:
    srv.chat("Bob", "#onuw", "!reveal");
    srv.settle().await;
    let second_reveal = srv.take_messages_to("#onuw").join("\n");
    assert!(second_reveal.contains("Nothing left to reveal from the last game!"));
    assert!(!second_reveal.contains("📜 What Happened:"));

    // 2. Start a second game with active Night prompt, verify `!reveal` is rejected mid-game,
    //    then `!abort` mid-Night and verify `+m` is cleaned up!
    srv.chat("Alice", "#onuw", "!onuw base");
    srv.settle().await;
    srv.chat("Bob", "#onuw", "!reveal");
    srv.settle().await;
    let mid_lobby_reveal = srv.take_messages_to("#onuw").join("\n");
    assert!(mid_lobby_reveal.contains("Finish the current game first"));
    srv.chat("Bob", "#onuw", "!join");
    srv.chat("Carol", "#onuw", "!join");
    srv.chat("Alice", "#onuw", "!clear");
    srv.chat("Alice", "#onuw", "!add 6x robber");
    srv.chat("Alice", "#onuw", "!go");
    srv.settle().await;

    // Drain prior non-message MODE/TOPIC commands, then abort via host `!abort`:
    while srv.try_recv_sent().is_some() {}
    srv.dm("Alice", "!abort");
    srv.settle().await;

    let mut saw_unmute = false;
    let mut saw_topic_restore = false;
    while let Some(cmd) = srv.try_recv_sent() {
        match cmd {
            Command::Raw(verb, args) if verb == "MODE" && args.get(1).map(String::as_str) == Some("-m") => {
                saw_unmute = true;
            }
            Command::TOPIC(_, Some(t)) if t == "Welcome to #onuw!" => {
                saw_topic_restore = true;
            }
            _ => {}
        }
    }
    assert!(saw_unmute, "expected ModerationGuard::drop to send MODE -m");
    assert!(
        saw_topic_restore,
        "expected ModerationGuard::drop to restore original topic"
    );

    bot.request_shutdown();
    bot_task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn recruit_players_default_config_skips_dm_check() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#onuw", &["Alice", "Bob", "Carol"]);
    srv.settle().await;

    let room = bot.room("#onuw").unwrap();
    let host = bot.user("Alice").unwrap();
    let cfg = GameConfig::default();

    let task = tokio::spawn({
        let room = room.clone();
        async move { recruit_players(&room, host, DeckSpec::default(), &cfg).await }
    });
    srv.settle().await;

    srv.chat("Bob", "#onuw", "!join");
    srv.chat("Carol", "#onuw", "!join");
    srv.chat("Alice", "#onuw", "!go");
    srv.settle().await;

    let (players, _) = task.await.unwrap().unwrap().expect("3 players recruited");
    let nicks: Vec<String> = players.iter().map(|u| u.nick()).collect();
    assert_eq!(nicks, vec!["Alice", "Bob", "Carol"]);
    assert!(srv.take_messages_to("Alice").is_empty());
    assert!(srv.take_messages_to("Bob").is_empty());
    assert!(srv.take_messages_to("Carol").is_empty());
    let chan_out = srv.take_messages_to("#onuw").join("\n");
    assert!(!chan_out.contains("DM check"));
}

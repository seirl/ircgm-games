use std::time::Duration;

use ircgm::irc::proto::Command;
use ircgm::testing::test_harness;
use ircgm_games_bot::{admin_quit_loop, run_lobby, EnabledGames};

#[tokio::test(start_paused = true)]
async fn lobby_starts_idle_switches_via_admin_game_command_and_delegates_onuw_controls() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#lobby", &["Alice", "Admin"]);
    srv.set_account("Admin", Some("adminacct"));
    srv.settle().await;

    let lobby = bot.room("#lobby").unwrap();
    let enabled = EnabledGames::all();
    let admins = vec!["adminacct".to_owned()];

    let task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        let admins = admins.clone();
        let enabled = enabled.clone();
        async move {
            tokio::try_join!(
                run_lobby(&bot, &lobby, &admins, &enabled),
                admin_quit_loop(&bot, &lobby, &admins),
            )
        }
    });
    srv.settle().await;

    // Bot starts idle without launching any game lobby automatically.
    let greeting = srv.take_messages_to("#lobby").join("\n");
    assert!(greeting.contains("Bot ready (idle)!"));
    assert!(greeting.contains("!game <name>"));
    assert!(!greeting.contains('`'));

    // Anyone can inspect `!games` or bare `!game`; non-admin `!game <name>` is ignored.
    srv.chat("Alice", "#lobby", "!games");
    srv.chat("Alice", "#lobby", "!game");
    srv.chat("Alice", "#lobby", "!game onuw");
    srv.settle().await;
    let status = srv.take_messages_to("#lobby").join("\n");
    assert!(status.contains("Active game: none (idle)"));
    assert!(status.contains("Enabled: onuw, secret_hitler"));
    assert!(!status.contains("One Night Ultimate Werewolf"));
    assert!(!status.contains('`'));

    // Unknown game reports error; `!game none` while already idle reports already idle;
    // `!stop` while idle reports no game is running.
    srv.chat("Admin", "#lobby", "!game nonexistent");
    srv.chat("Admin", "#lobby", "!game none");
    srv.chat("Admin", "#lobby", "!stop");
    srv.settle().await;
    let bad = srv.take_messages_to("#lobby").join("\n");
    assert!(bad.contains("Unknown game 'nonexistent'"));
    assert!(bad.contains("Bot is already idle."));
    assert!(bad.contains("No game is running."));
    assert!(!bad.contains('`'));

    // Admin loads ONUW lobby via `!game onuw` (shows `!start`, not `!onuw`).
    srv.chat("Admin", "#lobby", "!game onuw");
    srv.settle().await;
    let onuw_greeting = srv.take_messages_to("#lobby").join("\n");
    assert!(onuw_greeting.contains("One Night Ultimate Werewolf"));
    assert!(onuw_greeting.contains("!start"));
    assert!(!onuw_greeting.contains("!onuw"));
    assert!(!onuw_greeting.contains('`'));

    // Re-selecting the already active game does not restart it; `!stop` while game lobby is
    // idle reports no game is running without unloading the lobby.
    srv.chat("Admin", "#lobby", "!game onuw");
    srv.chat("Admin", "#lobby", "!stop");
    srv.settle().await;
    let idle_ctrl = srv.take_messages_to("#lobby").join("\n");
    assert!(idle_ctrl.contains("Already running onuw!"));
    assert!(idle_ctrl.contains("No game is running."));
    assert!(!idle_ctrl.contains("Closed onuw lobby"));

    // `!onuw` and `!fast` are removed; `!start` opens recruitment and gives open-lobby hints.
    srv.chat("Alice", "#lobby", "!onuw");
    srv.chat("Admin", "#lobby", "!fast");
    srv.settle().await;
    assert!(srv.take_messages_to("#lobby").is_empty());

    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    let started = srv.take_messages_to("#lobby").join("\n");
    assert!(!started.is_empty());
    assert!(!started.contains('`'));

    srv.chat("Alice", "#lobby", "!start vampire");
    srv.settle().await;
    let host_relaunch = srv.take_messages_to("#lobby").join("\n");
    assert!(host_relaunch.contains("!preset vampire"));
    assert!(!host_relaunch.contains("already in progress"));

    srv.chat("Admin", "#lobby", "!start");
    srv.settle().await;
    let non_host_relaunch = srv.take_messages_to("#lobby").join("\n");
    assert!(non_host_relaunch.contains("Lobby is already open"));
    assert!(non_host_relaunch.contains("!join"));

    srv.chat("Alice", "#lobby", "!abort");
    srv.settle().await;
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("aborted the ONUW game"));

    // Admin `!stop` cancels recruitment via DM; admin `!game none` also cleanly cancels an
    // unstarted recruitment lobby and returns to idle.
    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    srv.dm("Admin", "!stop");
    srv.settle().await;
    assert!(srv
        .take_messages_to("Admin")
        .join("\n")
        .contains("Stopped onuw lobby."));
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("onuw lobby was stopped by an admin."));

    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    srv.dm("Admin", "!game none");
    srv.settle().await;
    assert!(srv
        .take_messages_to("Admin")
        .join("\n")
        .contains("Closed onuw lobby in #lobby"));
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("Closed onuw lobby — bot is now idle."));

    srv.chat("Admin", "#lobby", "!quit");
    srv.settle().await;
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("Shutting down bot!"));

    let (_, admin_quit) = task.await.unwrap().unwrap();
    assert!(admin_quit);
}

#[tokio::test(start_paused = true)]
async fn lobby_protects_active_onuw_game_from_game_switch_until_complete_or_stop() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#lobby", &["Alice", "Bob", "Carol", "Admin"]);
    srv.set_account("Admin", Some("adminacct"));
    srv.send_line(":Alice!~Alice@test.host TOPIC #lobby :Original Topic\r\n");
    srv.settle().await;

    let lobby = bot.room("#lobby").unwrap();
    let enabled = EnabledGames::all();
    let admins = vec!["adminacct".to_owned()];

    let task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        async move { run_lobby(&bot, &lobby, &admins, &enabled).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    srv.chat("Admin", "#lobby", "!game onuw");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    // Before any game finishes, `!reveal` reports nothing to reveal.
    srv.chat("Alice", "#lobby", "!reveal");
    srv.settle().await;
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("Nothing left to reveal"));

    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    srv.chat("Bob", "#lobby", "!join");
    srv.chat("Carol", "#lobby", "!play");
    srv.chat("Alice", "#lobby", "!clear");
    srv.chat("Alice", "#lobby", "!add 6x insomniac");
    srv.chat("Alice", "#lobby", "!go");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    // While a dealt round is running, `!start` reports busy, `!games` reports in progress,
    // and admin `!game none` is REJECTED instead of interrupting the active match!
    srv.chat("Admin", "#lobby", "!start");
    srv.chat("Alice", "#lobby", "!games");
    srv.chat("Admin", "#lobby", "!game none");
    srv.dm("Admin", "!game none");
    srv.settle().await;
    let mid_game = srv.take_messages_to("#lobby").join("\n");
    assert!(mid_game.contains("already in progress"));
    assert!(mid_game.contains("game in progress"));
    assert!(mid_game.contains("A game of onuw is in progress — finish it or run !stop first!"));
    assert!(!mid_game.contains("Closed onuw lobby"));
    assert!(!mid_game.contains('`'));
    let admin_dm = srv.take_messages_to("Admin").join("\n");
    assert!(admin_dm.contains("A game of onuw is in progress — finish it or run !stop first!"));
    assert!(!admin_dm.contains('`'));

    // Default Night tier budget is 30s; the round finishes uninterrupted!
    srv.advance(Duration::from_secs(30)).await;

    srv.chat("Alice", "#lobby", "!ready");
    srv.chat("Bob", "#lobby", "!ready");
    srv.chat("Carol", "#lobby", "!ready");
    srv.settle().await;

    srv.chat("Alice", "#lobby", "!vote Bob");
    srv.chat("Bob", "#lobby", "!vote Carol");
    srv.chat("Carol", "#lobby", "!vote Alice");
    srv.settle().await;

    let post_vote = srv.take_messages_to("#lobby").join("\n");
    assert!(post_vote.contains("🏆 Winners: Alice (Village), Bob (Village), Carol (Village)"));
    assert!(post_vote.contains("!reveal"));
    assert!(post_vote.contains("!start"));
    assert!(!post_vote.contains("!onuw"));
    assert!(!post_vote.contains("📜 What Happened:"));

    // Back in Phase 1, `OnuwLobby` kept its `pending_reveal` across the phase transition!
    srv.chat("Carol", "#lobby", "!reveal");
    srv.settle().await;
    let revealed = srv.take_messages_to("#lobby").join("\n");
    assert!(revealed.contains("🗳️ VOTING REVEAL — Center:"));
    assert!(revealed.contains("📜 What Happened:"));
    assert!(revealed.contains("Insomniac (Alice) checked their card and saw Insomniac."));

    // Start a second game into Night (`+m` + topic set): `!game none` is rejected without
    // unmuting; then admin `!stop` ends the match cleanly (restoring `-m` & original topic),
    // after which `!game none` returns the bot to idle!
    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    srv.chat("Bob", "#lobby", "!join");
    srv.chat("Carol", "#lobby", "!play");
    srv.chat("Alice", "#lobby", "!clear");
    srv.chat("Alice", "#lobby", "!add 6x robber");
    srv.chat("Alice", "#lobby", "!go");
    srv.settle().await;
    while srv.try_recv_sent().is_some() {}

    srv.chat("Admin", "#lobby", "!game none");
    srv.settle().await;
    let mut unmuted_early = false;
    while let Some(cmd) = srv.try_recv_sent() {
        if let Command::Raw(verb, args) = cmd {
            if verb == "MODE" && args.get(1).map(String::as_str) == Some("-m") {
                unmuted_early = true;
            }
        }
    }
    assert!(
        !unmuted_early,
        "!game none must not interrupt or unmute an in-progress round"
    );

    srv.chat("Admin", "#lobby", "!stop");
    srv.settle().await;

    let mut saw_unmute = false;
    let mut saw_topic_restore = false;
    while let Some(cmd) = srv.try_recv_sent() {
        match cmd {
            Command::Raw(verb, args)
                if verb == "MODE" && args.get(1).map(String::as_str) == Some("-m") =>
            {
                saw_unmute = true;
            }
            Command::TOPIC(_, Some(t)) if t == "Original Topic" => {
                saw_topic_restore = true;
            }
            _ => {}
        }
    }
    assert!(saw_unmute, "expected MODE -m on admin !stop mid-game");
    assert!(
        saw_topic_restore,
        "expected original topic restored on admin !stop mid-game"
    );

    // Now that the round stopped, `!game none` switches back to idle cleanly.
    let _ = srv.take_messages_to("#lobby");
    srv.chat("Admin", "#lobby", "!game none");
    srv.settle().await;
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("Closed onuw lobby — bot is now idle."));

    bot.request_shutdown();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn admin_switches_between_onuw_and_secret_hitler_and_protects_active_sh_round() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#lobby", &["Alice", "Bob", "Carol", "Dave", "Eve", "Admin"]);
    srv.set_account("Admin", Some("adminacct"));
    srv.settle().await;

    let lobby = bot.room("#lobby").unwrap();
    let enabled = EnabledGames::all();
    let admins = vec!["adminacct".to_owned()];

    let task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        let admins = admins.clone();
        async move { run_lobby(&bot, &lobby, &admins, &enabled).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    // Load ONUW, open an unstarted recruitment lobby, and hot-switch directly to Secret Hitler:
    srv.chat("Admin", "#lobby", "!game onuw");
    srv.settle().await;
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("One Night Ultimate Werewolf"));

    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    srv.chat("Admin", "#lobby", "!game sh");
    srv.settle().await;
    let switched = srv.take_messages_to("#lobby").join("\n");
    assert!(switched.contains("Switching lobby from onuw to secret_hitler"));
    assert!(switched.contains("Secret Hitler"));
    assert!(switched.contains("!start"));
    assert!(switched.contains("!rules"));
    assert!(!switched.contains('`'));

    // Secret Hitler owns `!rules` and `!tracks`, ignores removed `!fast`, and admin `!stop`
    // while idle reports no game is running without closing the lobby:
    srv.chat("Alice", "#lobby", "!rules");
    srv.chat("Alice", "#lobby", "!tracks 5p");
    srv.chat("Admin", "#lobby", "!fast on");
    srv.chat("Admin", "#lobby", "!stop");
    srv.settle().await;
    let info = srv.take_messages_to("#lobby").join("\n");
    assert!(info.contains("Secret Hitler Rules"));
    assert!(info.contains("5-Player Secret Hitler"));
    assert!(info.contains("No game is running."));
    assert!(!info.contains("Fast mode"));
    assert!(!info.contains("Closed secret_hitler lobby"));
    assert!(!info.contains('`'));

    // Admin `!stop` cancels open recruitment while keeping Secret Hitler loaded:
    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");
    srv.chat("Admin", "#lobby", "!stop");
    srv.settle().await;
    assert!(srv
        .take_messages_to("#lobby")
        .join("\n")
        .contains("Stopped secret_hitler lobby."));

    // Start a 5-player game and verify open-lobby `!start` / `!reveal` hints and `!play` alias:
    srv.chat("Alice", "#lobby", "!start");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");
    srv.chat("Bob", "#lobby", "!start");
    srv.chat("Bob", "#lobby", "!reveal");
    srv.settle().await;
    let open_hints = srv.take_messages_to("#lobby").join("\n");
    assert!(open_hints.contains("Lobby is already open"));
    assert!(open_hints.contains("Finish the current game first"));

    srv.chat("Bob", "#lobby", "!join");
    srv.chat("Carol", "#lobby", "!play");
    srv.chat("Dave", "#lobby", "!join");
    srv.chat("Eve", "#lobby", "!play");
    srv.chat("Alice", "#lobby", "!go");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    // Once dealt, `!start` reports busy AND admin `!game onuw` / `!game none` are rejected
    // rather than interrupting the in-progress Secret Hitler match!
    srv.chat("Admin", "#lobby", "!start");
    srv.chat("Admin", "#lobby", "!game onuw");
    srv.chat("Admin", "#lobby", "!game none");
    srv.settle().await;
    let busy = srv.take_messages_to("#lobby").join("\n");
    assert!(busy.contains("A Secret Hitler game is already in progress"));
    assert!(busy.contains("A game of secret_hitler is in progress — finish it or run !stop first!"));
    assert!(!busy.contains("Switching lobby"));
    assert!(!busy.contains("Closed secret_hitler lobby"));
    assert!(!busy.contains('`'));

    // Default timers advance AFK votes through Chaos cleanly to game completion:
    let mut completed = false;
    for _ in 0..200 {
        srv.advance(Duration::from_secs(60)).await;
        let out = srv.take_messages_to("#lobby").join("\n");
        if out.contains("Game complete!") {
            assert!(out.contains("🏆 Winners:"));
            assert!(out.contains("!reveal"));
            assert!(out.contains("!start"));
            assert!(!out.contains("🃏 Secret Roles:"));
            assert!(!out.contains('`'));
            completed = true;
            break;
        }
    }
    assert!(completed, "expected Secret Hitler round to complete via Chaos");

    srv.chat("Alice", "#lobby", "!reveal");
    srv.settle().await;
    let revealed = srv.take_messages_to("#lobby").join("\n");
    assert!(revealed.contains("🃏 Secret Roles:"));
    assert!(revealed.contains("📜 What Happened:"));
    assert!(!revealed.contains('`'));

    // Now that the match is over, admin `!game onuw` switches cleanly back to ONUW!
    srv.chat("Admin", "#lobby", "!game onuw");
    srv.settle().await;
    let back_to_onuw = srv.take_messages_to("#lobby").join("\n");
    assert!(back_to_onuw.contains("Switching lobby from secret_hitler to onuw"));
    assert!(back_to_onuw.contains("One Night Ultimate Werewolf"));

    // Verify `--games onuw` prevents switching to a disabled game:
    bot.request_shutdown();
    task.await.unwrap().unwrap();

    let (bot2, mut srv2) = test_harness("GM");
    srv2.complete_join("#only-onuw", &["Admin"]);
    srv2.set_account("Admin", Some("adminacct"));
    srv2.settle().await;
    let lobby2 = bot2.room("#only-onuw").unwrap();
    let only_onuw = EnabledGames::from_specs(&["onuw"]).unwrap();
    let task2 = tokio::spawn({
        let bot2 = bot2.clone();
        let lobby2 = lobby2.clone();
        async move { run_lobby(&bot2, &lobby2, &admins, &only_onuw).await }
    });
    srv2.settle().await;
    let _ = srv2.take_messages_to("#only-onuw");

    srv2.chat("Admin", "#only-onuw", "!game sh");
    srv2.settle().await;
    assert!(srv2
        .take_messages_to("#only-onuw")
        .join("\n")
        .contains("secret_hitler is not enabled in this lobby"));

    bot2.request_shutdown();
    task2.await.unwrap().unwrap();
}

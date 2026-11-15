use std::time::Duration;

use ircgm::testing::test_harness;
use ircgm_games_bot::{admin_quit_loop, run_lobby, EnabledGames};

#[tokio::test(start_paused = true)]
async fn lobby_supports_games_catalog_admin_fast_busy_abort_stop_and_quit() {
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

    // Single-game ONUW greeting shows concise ONUW lobby help without markdown backticks.
    let greeting = srv.take_messages_to("#lobby").join("\n");
    assert!(greeting.contains("One Night Ultimate Werewolf"));
    assert!(greeting.contains("!onuw"));
    assert!(!greeting.contains('`'));

    // !games lists enabled games; unknown game reports error without backticks.
    srv.chat("Alice", "#lobby", "!games");
    srv.settle().await;
    let catalog = srv.take_messages_to("#lobby").join("\n");
    assert!(catalog.contains("Enabled games: onuw"));
    assert!(!catalog.contains('`'));

    srv.chat("Alice", "#lobby", "!play nonexistent");
    srv.settle().await;
    let unknown = srv.take_messages_to("#lobby").join("\n");
    assert!(unknown.contains("Unknown game 'nonexistent'"));
    assert!(!unknown.contains('`'));

    // Non-admin !fast is ignored; admin !fast toggles fast mode and accepts explicit on/off.
    srv.chat("Alice", "#lobby", "!fast");
    srv.settle().await;
    assert!(srv.take_messages_to("#lobby").is_empty());

    srv.chat("Admin", "#lobby", "!fast");
    srv.settle().await;
    let fast_on = srv.take_messages_to("#lobby").join("\n");
    assert!(fast_on.contains("Fast mode enabled"));

    srv.chat("Admin", "#lobby", "!fast off");
    srv.settle().await;
    let fast_off = srv.take_messages_to("#lobby").join("\n");
    assert!(fast_off.contains("Fast mode disabled"));

    srv.chat("Admin", "#lobby", "!fast maybe");
    srv.settle().await;
    let fast_bad = srv.take_messages_to("#lobby").join("\n");
    assert!(fast_bad.contains("Usage: !fast [on|off]"));
    assert!(!fast_bad.contains('`'));

    // Starting a game opens recruitment, gives open-lobby hints on concurrent launch attempts, and !abort cancels it.
    srv.chat("Alice", "#lobby", "!play onuw");
    srv.settle().await;
    let started = srv.take_messages_to("#lobby").join("\n");
    assert!(!started.is_empty());
    assert!(!started.contains('`'));

    srv.chat("Alice", "#lobby", "!onuw vampire");
    srv.settle().await;
    let host_relaunch = srv.take_messages_to("#lobby").join("\n");
    assert!(host_relaunch.contains("!preset vampire"));
    assert!(!host_relaunch.contains("already in progress"));

    srv.chat("Admin", "#lobby", "!play onuw");
    srv.settle().await;
    let non_host_relaunch = srv.take_messages_to("#lobby").join("\n");
    assert!(non_host_relaunch.contains("Lobby is already open"));
    assert!(non_host_relaunch.contains("!join"));
    assert!(!non_host_relaunch.contains('`'));

    srv.chat("Alice", "#lobby", "!abort");
    srv.settle().await;
    let aborted = srv.take_messages_to("#lobby").join("\n");
    assert!(aborted.contains("aborted onuw"));
    assert!(!aborted.contains('`'));

    // Admin !stop cancels an active game (including via DM), and admin !quit shuts down the bot cleanly.
    srv.chat("Alice", "#lobby", "!onuw");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    srv.dm("Admin", "!stop");
    srv.settle().await;
    let dm_reply = srv.take_messages_to("Admin").join("\n");
    assert!(dm_reply.contains("Stopped onuw"));
    assert!(!dm_reply.contains('`'));
    let chan_stopped = srv.take_messages_to("#lobby").join("\n");
    assert!(chan_stopped.contains("stopped by an admin"));

    srv.chat("Admin", "#lobby", "!quit");
    srv.settle().await;
    let quit_out = srv.take_messages_to("#lobby").join("\n");
    assert!(quit_out.contains("Shutting down bot!"));

    let (_, admin_quit) = task.await.unwrap().unwrap();
    assert!(admin_quit);
}

#[tokio::test(start_paused = true)]
async fn lobby_holds_back_onuw_chronicle_until_reveal_command() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#lobby", &["Alice", "Bob", "Carol", "Admin"]);
    srv.set_account("Admin", Some("adminacct"));
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

    // Before any game finishes, `!reveal` reports nothing to reveal.
    srv.chat("Alice", "#lobby", "!reveal");
    srv.settle().await;
    let empty_reveal = srv.take_messages_to("#lobby").join("\n");
    assert!(empty_reveal.contains("Nothing left to reveal"));

    // Enable fast mode so Night has no DM check and runs on 15s tiers.
    srv.chat("Admin", "#lobby", "!fast on");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");

    srv.chat("Alice", "#lobby", "!onuw");
    srv.settle().await;
    srv.chat("Bob", "#lobby", "!join");
    srv.chat("Carol", "#lobby", "!join");
    srv.chat("Alice", "#lobby", "!clear");
    srv.chat("Alice", "#lobby", "!add 6x insomniac");
    srv.chat("Alice", "#lobby", "!go");
    srv.settle().await;
    let _ = srv.take_messages_to("#lobby");
    srv.chat("Admin", "#lobby", "!play onuw");
    srv.settle().await;
    let mid_game_busy = srv.take_messages_to("#lobby").join("\n");
    assert!(mid_game_busy.contains("already in progress"));
    assert!(!mid_game_busy.contains('`'));

    srv.advance(Duration::from_secs(15)).await;

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
    assert!(!post_vote.contains("📜 What Happened:"));
    assert!(!post_vote.contains("🌅 DAWN REVEAL"));

    srv.chat("Carol", "#lobby", "!reveal");
    srv.settle().await;
    let revealed = srv.take_messages_to("#lobby").join("\n");
    assert!(revealed.contains("🌅 DAWN REVEAL — Center:"));
    assert!(revealed.contains("📜 What Happened:"));
    assert!(revealed.contains("Insomniac (Alice) checked their card and saw Insomniac."));

    srv.chat("Carol", "#lobby", "!reveal");
    srv.settle().await;
    let repeat_reveal = srv.take_messages_to("#lobby").join("\n");
    assert!(repeat_reveal.contains("Nothing left to reveal"));

    bot.request_shutdown();
    task.await.unwrap().unwrap();
}

use std::time::Duration;

use irc::proto::Command;
use ircgm::prelude::Text;
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

#[tokio::test(start_paused = true)]
async fn night_announces_canonical_order_and_anyone_can_call_order_command_during_day() {
    let (bot, mut srv) = test_harness("GM");
    // Dave and Eve are in `#onuw` as non-playing spectators alongside players Alice, Bob, Carol:
    srv.complete_join("#onuw", &["Alice", "Bob", "Carol", "Dave", "Eve"]);
    srv.settle().await;

    let lobby = bot.room("#onuw").unwrap();
    let cfg = GameConfig::for_tests();
    let bot_task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        async move { run_lobby_bot(&bot, &lobby, &[], &cfg).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#onuw");

    srv.chat("Alice", "#onuw", "!onuw");
    srv.settle().await;
    srv.chat("Bob", "#onuw", "!join");
    srv.chat("Carol", "#onuw", "!join");
    // Add roles in reverse order so deck order != canonical wake order:
    srv.chat("Alice", "#onuw", "!clear");
    srv.chat(
        "Alice",
        "#onuw",
        "!add insomniac drunk troublemaker seer villager villager",
    );
    srv.settle().await;
    let _ = srv.take_messages_to("#onuw");

    srv.chat("Alice", "#onuw", "!go");
    srv.settle().await;

    // Verify canonical role order is announced in channel right at the beginning of Night:
    let night_start_msgs: Vec<String> = srv
        .take_messages_to("#onuw")
        .into_iter()
        .map(|s| Text::parse(&s).plain())
        .collect();
    let order_pos = night_start_msgs
        .iter()
        .position(|m| m == "📋 Role order: Seer → Troublemaker → Drunk → Insomniac")
        .unwrap_or_else(|| {
            panic!(
                "expected canonical role order at start of Night, got:\n{}",
                night_start_msgs.join("\n")
            )
        });
    let night_falls_pos = night_start_msgs
        .iter()
        .position(|m| m.contains("Night falls!"))
        .expect("Night falls banner");
    assert!(
        order_pos < night_falls_pos,
        "expected role order before Night falls banner, got:\n{}",
        night_start_msgs.join("\n")
    );

    // Let the single Night tier time out (`tier_budget = 5s` in `GameConfig::for_tests`) so Day begins:
    srv.advance(Duration::from_secs(5)).await;
    let day_start = srv.take_messages_to("#onuw").join("\n");
    assert!(day_start.contains("EVERYONE WAKE UP!"));
    assert!(Text::parse(&day_start).plain().contains("!order"));
    assert!(!day_start.contains('`'));

    // Enrolled player, already-`!ready` player, channel spectator, and DM spectator can all call `!order`:
    srv.chat("Alice", "#onuw", "!order");
    srv.chat("Alice", "#onuw", "!ready");
    srv.chat("Alice", "#onuw", "!order");
    srv.chat("Dave", "#onuw", "!order");
    srv.dm("Eve", "!order");
    srv.settle().await;

    let chan_orders: Vec<String> = srv
        .take_messages_to("#onuw")
        .into_iter()
        .map(|s| Text::parse(&s).plain())
        .filter(|m| m.starts_with("📋"))
        .collect();
    assert_eq!(
        chan_orders,
        vec![
            "📋 Role order: Seer → Troublemaker → Drunk → Insomniac",
            "📋 Role order: Seer → Troublemaker → Drunk → Insomniac",
            "📋 Role order: Seer → Troublemaker → Drunk → Insomniac",
        ]
    );
    let eve_dms: Vec<String> = srv
        .take_messages_to("Eve")
        .into_iter()
        .map(|s| Text::parse(&s).plain())
        .collect();
    assert_eq!(
        eve_dms,
        vec!["📋 Role order: Seer → Troublemaker → Drunk → Insomniac"]
    );

    bot.request_shutdown();
    bot_task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn marks_and_artifacts_catalog_and_lookup_work_in_lobby_and_active_game() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#onuw", &["Alice", "Bob", "Carol"]);
    srv.settle().await;

    let lobby = bot.room("#onuw").unwrap();
    let cfg = GameConfig::for_tests();
    let bot_task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        async move { run_lobby_bot(&bot, &lobby, &[], &cfg).await }
    });
    srv.settle().await;
    let greeting = Text::parse(&srv.take_messages_to("#onuw").join("\n")).plain();
    assert!(greeting.contains("!marks"));
    assert!(greeting.contains("!artifacts"));
    assert!(!greeting.contains('`'));

    // Idle lobby: `!marks`, `!mark <name>`, `!artifacts`, `!artifact <name>`, and unknown errors:
    srv.chat("Alice", "#onuw", "!marks");
    srv.chat("Alice", "#onuw", "!mark fear");
    srv.chat("Alice", "#onuw", "!mark nope");
    srv.chat("Alice", "#onuw", "!artifacts");
    srv.chat("Alice", "#onuw", "!artifact shroud");
    srv.chat("Alice", "#onuw", "!artifact nope");
    srv.settle().await;

    let idle_out = Text::parse(&srv.take_messages_to("#onuw").join("\n")).plain();
    assert!(idle_out.contains("🔖 Marks (8): Mark of Clarity"));
    assert!(idle_out.contains("🔖 Mark of Fear · Vampire · The Count —"));
    assert!(idle_out.contains("Unknown mark 'nope' (try !marks)."));
    assert!(idle_out.contains("🏺 Artifacts (12 total) — !artifact <name> for details:"));
    assert!(idle_out.contains("Role-Replacing (8): Claw of the Werewolf"));
    assert!(idle_out.contains("Other Effects (4): Dagger of the Traitor"));
    assert!(idle_out.contains("🏺 Shroud of Shame · Daybreak (Curator) · Keeps role —"));
    assert!(idle_out.contains("Unknown artifact 'nope' (try !artifacts)."));
    assert!(!idle_out.contains('`'));

    // Open recruitment lobby + active game (channel & DM):
    srv.chat("Alice", "#onuw", "!onuw");
    srv.settle().await;
    let _ = srv.take_messages_to("#onuw");

    srv.chat("Bob", "#onuw", "!join");
    srv.chat("Carol", "#onuw", "!join");
    srv.chat("Bob", "#onuw", "!marks love");
    srv.chat("Carol", "#onuw", "!artifacts claw");
    srv.settle().await;
    let open_out = Text::parse(&srv.take_messages_to("#onuw").join("\n")).plain();
    assert!(open_out.contains("🔖 Mark of Love · Vampire · Cupid —"));
    assert!(open_out.contains("🏺 Claw of the Werewolf · Daybreak (Curator) · Replaces role —"));
    assert!(!open_out.contains('`'));

    srv.chat("Alice", "#onuw", "!clear");
    srv.chat("Alice", "#onuw", "!add 6x insomniac");
    srv.chat("Alice", "#onuw", "!go");
    srv.advance(Duration::from_secs(5)).await;
    let _ = srv.take_messages_to("#onuw");
    let _ = srv.take_messages_to("Bob");

    // During Day, channel & DM lookups work while unknown args do not spam channel:
    srv.chat("Alice", "#onuw", "!mark clarity");
    srv.chat("Alice", "#onuw", "!mark bogus");
    srv.dm("Bob", "!artifacts");
    srv.dm("Bob", "!artifact mute");
    srv.settle().await;

    let day_chan = Text::parse(&srv.take_messages_to("#onuw").join("\n")).plain();
    assert!(day_chan.contains("🔖 Mark of Clarity · Vampire · Initial / Priest —"));
    assert!(!day_chan.contains("Unknown mark"));
    assert!(!day_chan.contains('`'));

    let bob_dm = Text::parse(&srv.take_messages_to("Bob").join("\n")).plain();
    assert!(bob_dm.contains("🏺 Artifacts (12 total)"));
    assert!(bob_dm.contains("🏺 Mask of Muting · Daybreak (Curator) · Keeps role —"));
    assert!(!bob_dm.contains('`'));

    bot.request_shutdown();
    bot_task.await.unwrap().unwrap();
}

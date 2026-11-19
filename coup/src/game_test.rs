use std::time::Duration;

use irc::proto::Command;
use ircgm::prelude::{Error, Outcome, Text};
use ircgm::testing::test_harness;
use ircgm::GameFutureExt as _;

use crate::game::{run_live_game, run_single_game, GameConfig};
use crate::lobby::{handle_catalog_query, recruit_players};
use crate::state::{Allegiance, GameMode, Role};
use crate::testing::{CoupTestHarness, TEST_ROOM};

#[tokio::test(start_paused = true)]
async fn classic_coup_blocks_challenges_exchange_mandatory_coup_and_reveal() {
    let players = [
        ("Alice", [Role::Duke, Role::Ambassador], 2, None),
        ("Bob", [Role::Captain, Role::Contessa], 2, None),
        ("Carol", [Role::Assassin, Role::Duke], 2, None),
    ];
    let deck = [Role::Captain, Role::Contessa, Role::Assassin, Role::Duke];
    let mut h = CoupTestHarness::new(GameMode::classic(), &players, &deck, 0, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;

    let alice_dm = h.drain_dm_plain("Alice");
    assert!(alice_dm.contains("Your cards: 1: Duke · 2: Ambassador · Coins: 2c"));
    assert!(!alice_dm.contains('`'));
    let _ = h.drain_room_plain();

    // Turn 1 (Alice): Tax -> Carol challenges -> Alice proves Duke (swaps it!), Carol loses 1 card:
    h.srv.chat("Alice", TEST_ROOM, "!tax");
    h.srv.settle().await;
    h.srv.chat("Carol", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    // Carol has 2 distinct cards (Assassin, Duke); loses slot 1 (Assassin) in DM:
    h.srv.dm("Carol", "1");
    h.srv.settle().await;
    let out1 = h.drain_room_plain();
    assert!(out1.contains("Alice PROVED it and drew a replacement card"));
    assert!(out1.contains("Carol lost Assassin"));
    assert!(out1.contains("Alice collected Tax (+3c → 5c)"));
    assert!(!out1.contains('`'));

    // Turn 2 (Bob): Foreign Aid -> Carol claims Duke to block -> unchallenged (all pass):
    h.srv.chat("Bob", TEST_ROOM, "!aid");
    h.srv.settle().await;
    h.srv.chat("Carol", TEST_ROOM, "!block");
    h.srv.settle().await;
    h.pass_all(&["Alice", "Bob"]).await;
    let out2 = h.drain_room_plain();
    assert!(out2.contains("Carol's Duke block stands!"));

    // Turn 3 (Carol, 1 live card = Duke): bluffs Steal against Bob -> Alice tries to block
    // (rejected: only target can block!), Bob blocks with Captain -> Carol challenges -> Bob proves!
    // Carol has only 1 card left so she is automatically eliminated!
    h.srv.chat("Carol", TEST_ROOM, "!steal Bob");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!block captain");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("Only Bob (the target) can block this action!"));

    h.srv.chat("Bob", TEST_ROOM, "!block captain");
    h.srv.settle().await;
    h.srv.chat("Carol", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    let out3 = h.drain_room_plain();
    assert!(out3.contains("Bob PROVED it"));
    assert!(out3.contains("Carol lost their last influence (Duke)"));
    assert!(out3.contains("eliminated"));

    // Dead player parting is cleanly ignored!
    h.srv.part_user("Carol", TEST_ROOM, Some("gg"));
    h.srv.settle().await;

    // Turn 4 (Alice, 5c): Exchange (Ambassador) -> Bob passes -> Alice picks cards 1 & 2 in DM:
    h.srv.chat("Alice", TEST_ROOM, "!exchange");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.dm("Alice", "1 2");
    h.srv.settle().await;
    assert!(h.drain_room_plain().contains("Alice completed Exchange"));

    // Turn 5 (Bob, 2c): Income -> 3c.
    h.srv.chat("Bob", TEST_ROOM, "!income");
    h.srv.settle().await;

    // Turn 6 (Alice, 5c): Tax -> 8c; Turn 7 (Bob, 3c): Income -> 4c; Turn 8 (Alice, 8c): Tax -> 11c:
    h.srv.chat("Alice", TEST_ROOM, "!tax");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.chat("Bob", TEST_ROOM, "!income");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!tax");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.chat("Bob", TEST_ROOM, "!income");
    h.srv.settle().await;
    let _ = h.drain_room_plain();

    // Turn 10 (Alice, 11c >= 10): mandatory Coup! `!income` is rejected, `!coup Bob` succeeds:
    h.srv.chat("Alice", TEST_ROOM, "!income");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("You have 10+ coins and MUST launch a Coup"));

    h.srv.chat("Alice", TEST_ROOM, "!coup Bob");
    h.srv.settle().await;
    h.srv.dm("Bob", "1");
    h.srv.settle().await;

    // Bob (1 card left, 5c): Income -> 6c; Alice (4c): Tax -> 7c; Bob: Income -> 7c; Alice Coups Bob -> Alice wins!
    h.srv.chat("Bob", TEST_ROOM, "!income");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!tax");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.chat("Bob", TEST_ROOM, "!income");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!coup Bob");
    h.srv.settle().await;

    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(summary.winner, 0);
    let end_out = h.drain_room_plain();
    assert!(end_out.contains("Alice wins Coup!"));
    assert!(!end_out.contains('`'));

    let reveal_text = summary
        .reveal
        .lines()
        .iter()
        .map(Text::plain)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(reveal_text.contains("Final Cards:"));
    assert!(reveal_text.contains("What Happened:"));
    assert!(!reveal_text.contains('`'));
}

#[tokio::test(start_paused = true)]
async fn assassination_refund_vs_blocked_cost_and_post_challenge_double_loss() {
    // 3 players: Alice (Duke, Captain, 6c), Bob (Assassin, Duke, 6c), Carol (Duke, Captain, 2c).
    let players = [
        ("Alice", [Role::Duke, Role::Captain], 6, None),
        ("Bob", [Role::Assassin, Role::Duke], 6, None),
        ("Carol", [Role::Duke, Role::Captain], 2, None),
    ];
    let deck = [Role::Contessa, Role::Ambassador, Role::Duke];
    let mut h = CoupTestHarness::new(GameMode::classic(), &players, &deck, 0, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;
    let _ = h.drain_room_plain();

    // 1. Alice BLUFFS Assassinate on Carol (pays 3c -> 3c), Carol challenges -> Alice caught!
    // Official rule: 3c MUST be REFUNDED to Alice (back to 6c) and Alice loses 1 card!
    h.srv.chat("Alice", TEST_ROOM, "!assassinate Carol");
    h.srv.settle().await;
    h.srv.chat("Carol", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    h.srv.dm("Alice", "2"); // Alice loses Captain, keeps Duke
    h.srv.settle().await;
    let out1 = h.drain_room_plain();
    assert!(out1.contains("Alice was BLUFFING!"));
    assert!(out1.contains("Alice's 3c Assassination fee is refunded."));
    assert!(out1.contains("Alice (6c,"));

    // 2. Bob (real Assassin, 6c) assassinates Alice (pays 3c -> 3c); Alice bluffs Contessa block;
    //    Bob & Carol pass -> block stands -> Official rule: Bob's 3c is NOT refunded (stays 3c)!
    h.srv.chat("Bob", TEST_ROOM, "!assassinate Alice");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!contessa");
    h.srv.settle().await;
    h.pass_all(&["Bob", "Carol"]).await;
    let out2 = h.drain_room_plain();
    assert!(out2.contains("Alice's Contessa block stands!"));
    assert!(out2.contains("Bob (3c,"));

    // 3. Carol takes Income. Alice takes Income.
    h.srv.chat("Carol", TEST_ROOM, "!income");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!income");
    h.srv.settle().await;
    let _ = h.drain_room_plain();

    // 4. Bob (real Assassin, 3c) assassinates Carol (2 live cards: Duke, Captain).
    //    Carol challenges Bob's Assassin -> Bob PROVES Assassin!
    //    First loss: Carol loses Duke (DM "1").
    //    Official rule: Carol STILL gets a Block Window with her remaining card!
    //    Carol bluffs Contessa -> Alice (third party!) challenges Carol's Contessa -> Carol caught!
    //    Second loss: Carol loses her last card (Captain) -> eliminated!
    h.srv.chat("Bob", TEST_ROOM, "!assassinate Carol");
    h.srv.settle().await;
    h.srv.chat("Carol", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    h.srv.dm("Carol", "1"); // loses Duke, keeps Captain
    h.srv.settle().await;
    let mid = h.drain_room_plain();
    assert!(mid.contains("Bob PROVED it"));
    assert!(mid.contains("Carol lost Duke"));
    assert!(mid.contains("Carol, you may still !block (Contessa)"));

    h.srv.chat("Carol", TEST_ROOM, "!block");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    let dbl = h.drain_room_plain();
    assert!(dbl.contains("Carol was BLUFFING!"));
    assert!(dbl.contains("Carol lost their last influence (Captain)"));
    assert!(dbl.contains("eliminated"));

    // Now Alice (7c, 1 live card) Coups Bob (2 live cards), then Bob takes Income / Alice takes Income...
    // Actually let Alice Coup Bob (1st card), then finish via AFK or clean Coup:
    h.srv.chat("Alice", TEST_ROOM, "!coup Bob");
    h.srv.settle().await;
    h.srv.dm("Bob", "1");
    h.srv.settle().await;
    // Bob (0c, 1 card) bluffs Assassinate? Needs 3c; let Bob bluff Tax and Alice challenge if Bob has no Duke,
    // or simply have Bob lose via Alice Stealing -> Bob bluffs -> challenge!
    // Bob currently holds 1 card; let's check via Bob DM or just advance AFK to completion!
    for _ in 0..80 {
        if game_task.is_finished() {
            break;
        }
        h.srv.advance(Duration::from_secs(5)).await;
    }
    let summary = game_task.await.unwrap().unwrap();
    assert!(summary.winner == 0 || summary.winner == 1);
}

#[tokio::test(start_paused = true)]
async fn inquisitor_examine_forced_swap_keep_exchange_and_steal_block() {
    let players = [
        ("Alice", [Role::Inquisitor, Role::Duke], 2, None),
        ("Bob", [Role::Contessa, Role::Captain], 2, None),
    ];
    // Only 1 card in deck (Assassin) so forced Examine MUST give Bob Assassin (drawn BEFORE returning Contessa)!
    let deck = [Role::Assassin];
    let mut h = CoupTestHarness::new(GameMode::inquisitor(), &players, &deck, 0, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;
    let _ = h.drain_room_plain();

    // 1. Alice uses Inquisitor Examine on Bob -> Bob passes -> Bob chooses card 1 (Contessa) in DM
    //    -> Alice replies `!force` in DM -> Bob MUST draw Assassin and Contessa returns to deck!
    h.srv.chat("Alice", TEST_ROOM, "!examine Bob");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.dm("Bob", "1"); // show Contessa
    h.srv.settle().await;
    assert!(h
        .drain_dm_plain("Alice")
        .contains("Bob showed you: Contessa"));
    h.srv.dm("Alice", "!force");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("Alice forced Bob to swap their examined card"));
    assert!(h
        .drain_dm_plain("Bob")
        .contains("forced you to replace Contessa — you drew: Assassin"));

    // 2. Bob Steals from Alice -> Alice blocks with Inquisitor -> Bob passes -> blocked!
    h.srv.chat("Bob", TEST_ROOM, "!steal Alice");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!block inq");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    assert!(h
        .drain_room_plain()
        .contains("Alice's Inquisitor block stands!"));

    // 3. Alice uses Inquisitor Exchange (draws 1 card: Contessa) -> keeps cards 1 & 3 (Inquisitor, Contessa):
    h.srv.chat("Alice", TEST_ROOM, "!exchange");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.dm("Alice", "1 3");
    h.srv.settle().await;
    assert!(h
        .drain_dm_plain("Alice")
        .contains("1: Inquisitor · 2: Contessa"));

    // 4. Bob Income; Alice Examine Bob with `!keep`; then AFK to finish cleanly:
    h.srv.chat("Bob", TEST_ROOM, "!income");
    h.srv.settle().await;
    h.srv.chat("Alice", TEST_ROOM, "!examine Bob");
    h.srv.settle().await;
    h.pass_all(&["Bob"]).await;
    h.srv.dm("Bob", "2");
    h.srv.settle().await;
    h.srv.dm("Alice", "!keep");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("Alice allowed Bob to keep their examined card"));

    for _ in 0..100 {
        if game_task.is_finished() {
            break;
        }
        h.srv.advance(Duration::from_secs(5)).await;
    }
    let _summary = game_task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn reformation_allegiances_convert_embezzle_honest_and_caught_and_all_same_lift() {
    let players = [
        (
            "Alice",
            [Role::Captain, Role::Contessa],
            3,
            Some(Allegiance::Loyalist),
        ),
        (
            "Bob",
            [Role::Duke, Role::Assassin],
            3,
            Some(Allegiance::Loyalist),
        ),
        (
            "Carol",
            [Role::Ambassador, Role::Contessa],
            2,
            Some(Allegiance::Reformist),
        ),
    ];
    let deck = [
        Role::Captain,
        Role::Assassin,
        Role::Ambassador,
        Role::Contessa,
    ];
    let mut h = CoupTestHarness::new(GameMode::reformation(), &players, &deck, 0, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;
    let _ = h.drain_room_plain();

    // 1. Alice (Loyalist) cannot Steal from Bob (Loyalist), so Alice Converts self (1c -> Treasury = 1c, now Reformist):
    h.srv.chat("Alice", TEST_ROOM, "!steal Bob");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("cannot target a player of the same Allegiance"));

    h.srv.chat("Alice", TEST_ROOM, "!convert");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("Alice paid 1c to Treasury (now 1c) and converted to Reformist!"));

    // 2. Bob (Loyalist, HAS Duke!) pays 2c to Convert Carol (Reformist -> Loyalist; Treasury now 3c):
    h.srv.chat("Bob", TEST_ROOM, "!convert Carol");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("Bob paid 2c to Treasury (now 3c) and converted Carol to Loyalist!"));

    // 3. Carol (Loyalist, NO Duke) tries Foreign Aid -> Bob (Loyalist) CANNOT block teammate's Foreign Aid!
    h.srv.chat("Carol", TEST_ROOM, "!aid");
    h.srv.settle().await;
    h.srv.chat("Bob", TEST_ROOM, "!block");
    h.srv.settle().await;
    assert!(h
        .drain_room_plain()
        .contains("cannot block a teammate's Foreign Aid"));
    h.pass_all(&["Alice"]).await;
    let _ = h.drain_room_plain();

    // 4. Alice (Reformist, NO Duke) Embezzles Treasury (3c) -> Bob challenges -> Alice PROVES no Duke
    //    (reveals & swaps both live cards!), Bob loses 1 card, Alice gets 3c (Treasury = 0c)!
    h.srv.chat("Alice", TEST_ROOM, "!embezzle");
    h.srv.settle().await;
    h.srv.chat("Bob", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    h.srv.dm("Bob", "2"); // Bob loses Assassin, KEEPS Duke!
    h.srv.settle().await;
    let emb_ok = h.drain_room_plain();
    assert!(emb_ok.contains("no Duke) and replaced 2 card(s)"));
    assert!(emb_ok.contains("Bob lost Assassin"));
    assert!(emb_ok.contains("Alice embezzled 3c from the Treasury Reserve"));

    // 5. Bob (Loyalist, 1c, STILL holds Duke!) converts self (1c -> Treasury = 1c, now Reformist).
    h.srv.chat("Bob", TEST_ROOM, "!convert");
    h.srv.settle().await;
    // Carol (Loyalist) converts self (1c -> Treasury = 2c, now Reformist -> ALL 3 ARE REFORMIST!).
    h.srv.chat("Carol", TEST_ROOM, "!convert");
    h.srv.settle().await;
    // Alice (Reformist) takes Income (all same allegiance now!).
    h.srv.chat("Alice", TEST_ROOM, "!income");
    h.srv.settle().await;
    let _ = h.drain_room_plain();

    // 6. Bob (Reformist, 1 live card = Duke!) BLUFFS Embezzle -> Carol (ALSO Reformist! Challenges
    //    always allowed across teammates!) challenges -> Bob caught with Duke -> eliminated!
    h.srv.chat("Bob", TEST_ROOM, "!embezzle");
    h.srv.settle().await;
    h.srv.chat("Carol", TEST_ROOM, "!challenge");
    h.srv.settle().await;
    let emb_fail = h.drain_room_plain();
    assert!(emb_fail.contains("Bob DOES hold a Duke! Treasury stays at 2c."));
    assert!(emb_fail.contains("Bob lost their last influence (Duke)"));

    // Now Alice & Carol are both Reformist -> all-same-allegiance exception allows Coup/Steal -> finish via AFK!
    for _ in 0..100 {
        if game_task.is_finished() {
            break;
        }
        h.srv.advance(Duration::from_secs(5)).await;
    }
    let summary = game_task.await.unwrap().unwrap();
    assert!(summary.winner == 0 || summary.winner == 2);
    assert_eq!(summary.final_board.treasury, 2);
}

#[tokio::test(start_paused = true)]
async fn two_player_official_variant_draft_starting_coins_and_three_card_deck() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#coup2", &["Alice", "Bob"]);
    srv.settle().await;

    let room = bot.room("#coup2").unwrap();
    let cfg = GameConfig::for_tests();
    let players = vec![bot.user("Alice").unwrap(), bot.user("Bob").unwrap()];

    let game_task = tokio::spawn({
        let bot = bot.clone();
        let room = room.clone();
        let cfg = cfg.clone();
        async move { run_live_game(&bot, &room, players, GameMode::classic(), &cfg).await }
    });
    srv.settle().await;

    let draft_banner = Text::parse(&srv.take_messages_to("#coup2").join("\n")).plain();
    assert!(draft_banner.contains("2-Player Official Variant"));
    assert!(!draft_banner.contains('`'));

    // Both players pick their first card in DM:
    srv.dm("Alice", "duke");
    srv.dm("Bob", "contessa");
    srv.settle().await;

    let start_out = Text::parse(&srv.take_messages_to("#coup2").join("\n")).plain();
    assert!(start_out.contains("Classic Coup begins! (2 players, 3 cards in Court Deck)"));
    // Official 2p starting coins: first seat has 1c, second seat has 2c:
    assert!(start_out.contains("(1c, 2♥)"));
    assert!(start_out.contains("(2c, 2♥)"));

    let alice_dm = Text::parse(&srv.take_messages_to("Alice").join("\n")).plain();
    let bob_dm = Text::parse(&srv.take_messages_to("Bob").join("\n")).plain();
    assert!(alice_dm.contains("1: Duke"));
    assert!(bob_dm.contains("1: Contessa"));

    // AFK timers finish the 2p game cleanly (Income -> 10+ auto-Coup) without deadlocking:
    for _ in 0..120 {
        if game_task.is_finished() {
            break;
        }
        srv.advance(Duration::from_secs(5)).await;
    }
    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(summary.final_board.deck.len(), 3);

    // Regression: parting the channel during the 2-player starting card draft aborts immediately
    // with `Error::UserLeft` and restores `-m` and the original topic without waiting for timeout!
    srv.send_line(":Alice!~Alice@test.host TOPIC #coup2 :2P Topic\r\n");
    srv.settle().await;
    let draft_players = vec![bot.user("Alice").unwrap(), bot.user("Bob").unwrap()];
    let draft_task = tokio::spawn({
        let bot = bot.clone();
        let room = room.clone();
        async move { run_live_game(&bot, &room, draft_players, GameMode::classic(), &cfg).await }
    });
    srv.settle().await;
    while srv.try_recv_sent().is_some() {}

    srv.part_user("Bob", "#coup2", Some("brb"));
    srv.settle().await;
    assert!(matches!(
        draft_task.await.unwrap(),
        Err(Error::UserLeft { ref nick, .. }) if nick == "Bob"
    ));

    let mut saw_unmute = false;
    let mut saw_topic = false;
    while let Some(cmd) = srv.try_recv_sent() {
        match cmd {
            Command::Raw(verb, args)
                if verb == "MODE" && args.get(1).map(String::as_str) == Some("-m") =>
            {
                saw_unmute = true;
            }
            Command::TOPIC(_, Some(t)) if t == "2P Topic" => {
                saw_topic = true;
            }
            _ => {}
        }
    }
    assert!(
        saw_unmute,
        "expected MODE -m when player parts during 2p draft"
    );
    assert!(
        saw_topic,
        "expected original topic restored when player parts during 2p draft"
    );
}

#[tokio::test(start_paused = true)]
async fn lobby_recruitment_play_alias_mode_switch_live_queries_departure_and_abort_restore() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#coup", &["Alice", "Bob", "Carol"]);
    srv.send_line(":Alice!~Alice@test.host TOPIC #coup :Welcome to #coup!\r\n");
    srv.settle().await;

    let room = bot.room("#coup").unwrap();
    let cfg = GameConfig::for_tests();

    // 1. Open recruitment, test `!rules`, `!roles`, `!mode`, `!play` alias, host handoff, `< 2` `!go`:
    let recruit_task = tokio::spawn({
        let room = room.clone();
        let host = bot.user("Alice").unwrap();
        let cfg = cfg.clone();
        async move { recruit_players(&room, host, GameMode::classic(), &cfg).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#coup");

    srv.chat("Alice", "#coup", "!rules");
    srv.chat("Alice", "#coup", "!roles");
    srv.chat("Alice", "#coup", "!mode reformation inquisition");
    srv.chat("Alice", "#coup", "!go"); // only 1 player -> rejected
    srv.chat("Bob", "#coup", "!play");
    srv.chat("Alice", "#coup", "!leave");
    srv.chat("Carol", "#coup", "!join");
    srv.chat("Bob", "#coup", "!players");
    srv.chat("Bob", "#coup", "!help");
    srv.chat("Bob", "#coup", "!go");
    srv.settle().await;

    let (players, mode) = recruit_task.await.unwrap().unwrap().expect("2 players");
    assert_eq!(players.len(), 2);
    assert_eq!(mode, GameMode::reformation_inquisitor());
    let out = Text::parse(&srv.take_messages_to("#coup").join("\n")).plain();
    assert!(out.contains("Coup Rules"));
    assert!(out.contains("Mode switched to Coup: Reformation + Inquisitor!"));
    assert!(out.contains("Need at least 2 players"));
    assert!(out.contains("Alice left — Bob is now lobby host!"));
    assert!(out.contains("Open Lobby"));
    assert!(!out.contains('`'));

    // Also verify `handle_catalog_query` ignores unrelated commands:
    let mut inbox = room.inbox();
    srv.chat("Alice", "#coup", "!tracks");
    srv.settle().await;
    let msg = inbox.next_chat().await.unwrap();
    assert!(!handle_catalog_query(&room, &msg, None).unwrap());

    // 2. Run a 3-player game via `run_single_game`, test live queries, then living player parts ->
    //    returns `Error::UserLeft` and restores `-m` + original topic:
    let game_task = tokio::spawn({
        let bot = bot.clone();
        let room = room.clone();
        let host = bot.user("Alice").unwrap();
        let cfg = cfg.clone();
        async move { run_single_game(&bot, &room, host, GameMode::inquisitor(), &cfg).await }
    });
    srv.settle().await;
    srv.chat("Bob", "#coup", "!play");
    srv.chat("Carol", "#coup", "!join");
    srv.chat("Alice", "#coup", "!go");
    srv.settle().await;
    let _ = srv.take_messages_to("#coup");

    srv.chat("Alice", "#coup", "!board");
    srv.chat("Alice", "#coup", "!players");
    srv.chat("Alice", "#coup", "!hand");
    srv.dm("Alice", "!hand");
    srv.chat("Alice", "#coup", "!time");
    srv.chat("Alice", "#coup", "!help");
    srv.settle().await;
    let active_out = Text::parse(&srv.take_messages_to("#coup").join("\n")).plain();
    assert!(active_out.contains("Turn 1"));
    assert!(active_out.contains("Table order (3/3 alive)"));
    assert!(active_out.contains("Send !hand in DM"));
    assert!(active_out.contains("remaining in Action"));
    assert!(active_out.contains("!examine <nick>"));
    assert!(!active_out.contains('`'));
    assert!(Text::parse(&srv.take_messages_to("Alice").join("\n"))
        .plain()
        .contains("Your cards:"));

    while srv.try_recv_sent().is_some() {}
    srv.part_user("Carol", "#coup", Some("bye"));
    srv.settle().await;
    assert!(matches!(
        game_task.await.unwrap(),
        Err(Error::UserLeft { ref nick, .. }) if nick == "Carol"
    ));

    let mut saw_unmute = false;
    let mut saw_topic = false;
    while let Some(cmd) = srv.try_recv_sent() {
        match cmd {
            Command::Raw(verb, args)
                if verb == "MODE" && args.get(1).map(String::as_str) == Some("-m") =>
            {
                saw_unmute = true;
            }
            Command::TOPIC(_, Some(t)) if t == "Welcome to #coup!" => {
                saw_topic = true;
            }
            _ => {}
        }
    }
    assert!(saw_unmute, "expected MODE -m on living player departure");
    assert!(saw_topic, "expected original topic restored on departure");

    // 3. Rejoin Carol, start another game, and cancel mid-game via `.until(...)` -> restores `-m`:
    srv.join_user("Carol", "#coup");
    srv.settle().await;
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let abort_task = tokio::spawn({
        let bot = bot.clone();
        let room = room.clone();
        let host = bot.user("Alice").unwrap();
        async move {
            run_single_game(&bot, &room, host, GameMode::classic(), &cfg)
                .until(async {
                    let _ = cancel_rx.await;
                })
                .await
        }
    });
    srv.settle().await;
    srv.chat("Bob", "#coup", "!play");
    srv.chat("Carol", "#coup", "!play");
    srv.chat("Alice", "#coup", "!go");
    srv.settle().await;
    while srv.try_recv_sent().is_some() {}

    let _ = cancel_tx.send(());
    assert!(matches!(abort_task.await.unwrap(), Outcome::Stopped(())));
    srv.settle().await;

    let mut saw_abort_unmute = false;
    while let Some(cmd) = srv.try_recv_sent() {
        if let Command::Raw(verb, args) = cmd {
            if verb == "MODE" && args.get(1).map(String::as_str) == Some("-m") {
                saw_abort_unmute = true;
            }
        }
    }
    assert!(saw_abort_unmute, "expected MODE -m on cancellation/abort");
}

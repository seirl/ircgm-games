use std::time::Duration;

use irc::proto::Command;
use ircgm::prelude::{Error, Outcome, Text};
use ircgm::testing::test_harness;
use ircgm::GameFutureExt as _;

use crate::game::{run_single_game, GameConfig};
use crate::lobby::{handle_catalog_query, recruit_players};
use crate::state::{Party, Policy, Role, WinReason};
use crate::testing::{ShTestHarness, TEST_ROOM};

#[tokio::test(start_paused = true)]
async fn five_player_visibility_peek_reshuffle_dead_departure_and_hitler_execution() {
    // 5 players: Alice (Lib), Bob (Lib), Carol (Lib), Dave (Fascist), Eve (Hitler).
    // Construct a 9-card test deck so after 2 legislative sessions (6 cards drawn -> 3 left) and
    // a 3rd legislative session (3 cards drawn -> 0 left < 3), reshuffle fires BEFORE Policy Peek!
    let deals = [
        ("Alice", Role::Liberal),
        ("Bob", Role::Liberal),
        ("Carol", Role::Liberal),
        ("Dave", Role::Fascist),
        ("Eve", Role::Hitler),
    ];
    let deck = [
        // Gov 1 (Alice + Bob): draws [F, F, F] -> enacts F #1 (2 F in discard)
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // Gov 2 (Bob + Carol): draws [F, F, F] -> enacts F #2 (4 F in discard)
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // Gov 3 (Carol + Dave): draws [F, L, L] -> enacts F #3 (4 F + 2 L = 6 in discard, 0 in draw -> reshuffles 6!)
        // Wait: to keep subsequent draws 100% deterministic even after shuffle, put ONLY Fascist
        // cards in discard! Let's make all 9 initial cards Fascist:
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
    ];

    let mut h = ShTestHarness::new(&deals, &deck, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;

    // 1. Check 5p role visibility DMs (Hitler DOES know Fascist in 5–6p!) and zero backticks:
    let alice_dm = h.take_dm_plain("Alice");
    let dave_dm = h.take_dm_plain("Dave");
    let eve_dm = h.take_dm_plain("Eve");
    assert!(alice_dm.contains("Your Secret Role: LIBERAL"));
    assert!(!alice_dm.contains("Dave"));
    assert!(!alice_dm.contains("Eve"));
    assert!(dave_dm.contains("Your Secret Role: FASCIST"));
    assert!(dave_dm.contains("Hitler is Eve (Hitler knows who you are)"));
    assert!(eve_dm.contains("Your Secret Role: HITLER"));
    assert!(eve_dm.contains("Your Fascist teammate is Dave"));
    assert!(!alice_dm.contains('`') && !dave_dm.contains('`') && !eve_dm.contains('`'));

    let start_chan = h.take_channel_raw();
    assert!(!start_chan.contains('`'));

    // Round 1: Alice nominates Bob -> 5 Ja -> F #1 enacted (no power in 5p):
    h.chat("Alice", "!chancellor Bob").await;
    h.vote_all(&[
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ])
    .await;
    h.dm("Alice", "fascist").await;
    h.dm("Bob", "fascist").await;

    // Round 2 (5 living players -> 5-player rule: last Pres Alice IS eligible, last Chan Bob is NOT!):
    // Current Pres is Bob; Alice should be eligible!
    h.chat("Bob", "!chancellor Alice").await;
    h.vote_all(&[
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ])
    .await;
    h.dm("Bob", "f").await;
    h.dm("Alice", "f").await;

    // Round 3: Carol nominates Dave -> enacts F #3 -> draw pile hits 0 (< 3), so discard (6 F)
    // MUST reshuffle BEFORE Carol's Policy Peek!
    let _ = h.take_dm_plain("Carol");
    h.chat("Carol", "!chancellor Dave").await;
    h.vote_all(&[
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ])
    .await;
    let _ = h.take_dm_plain("Carol");
    h.dm("Carol", "discard fascist").await;
    h.dm("Dave", "enact fascist").await;

    let r3_chan = h.take_channel_plain();
    let reshuffle_pos = r3_chan
        .find("reshuffled with discard pile (6 policies in deck)")
        .expect("expected reshuffle announcement");
    let peek_pos = r3_chan
        .find("Policy Peek — Carol privately examines")
        .expect("expected Policy Peek announcement");
    assert!(
        reshuffle_pos < peek_pos,
        "reshuffle must occur before Policy Peek:\n{r3_chan}"
    );
    let carol_peek_dm = h.take_dm_plain("Carol");
    assert!(
        carol_peek_dm.contains("Policy Peek — Top 3 policies in draw pile (first → third): [FASCIST, FASCIST, FASCIST]"),
        "unexpected Carol DM:\n{carol_peek_dm}"
    );

    // Round 4: Dave nominates Carol (Alice wasn't last Chan; Dave was last Chan, wait: Dave IS
    // current Pres now, so everyone else is eligible!) -> enacts F #4 -> Dave executes Bob (NOT Hitler):
    h.chat("Dave", "!chancellor Carol").await;
    h.vote_all(&[
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ])
    .await;
    h.dm("Dave", "f").await;
    h.dm("Carol", "f").await;
    h.chat("Dave", "!kill Bob").await;

    let kill_chan = h.take_channel_plain();
    assert!(kill_chan.contains("Dave formally executes Bob! Bob was NOT Hitler"));
    assert!(!kill_chan.contains("Liberal"));

    // Executed player Bob parts the channel — game MUST NOT abort because Bob is already dead!
    h.srv.part_user("Bob", TEST_ROOM, Some("I am dead"));
    h.srv.settle().await;
    assert!(!game_task.is_finished());

    // Round 5: Eve is Pres! Wait: Eve is Hitler, let's fail Eve's election once so Alice becomes Pres
    // and executes Eve!
    h.chat("Eve", "!chancellor Alice").await;
    h.vote_all(&[
        ("Alice", "nein"),
        ("Carol", "nein"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ])
    .await;
    // Tie 2 Ja / 2 Nein fails! Next Pres is Alice (Bob is dead and skipped!):
    h.chat("Alice", "!chancellor Dave").await;
    h.vote_all(&[
        ("Alice", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "nein"),
    ])
    .await;
    h.dm("Alice", "f").await;
    h.dm("Dave", "f").await;

    // 5th Fascist policy unlocks Veto AND grants Execution -> Alice executes Eve (Hitler)!
    let pre_exec_chan = h.take_channel_plain();
    assert!(pre_exec_chan.contains("Veto Power is now permanently unlocked"));
    h.chat("Alice", "!kill Eve").await;

    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(
        summary.win_reason,
        WinReason::HitlerExecuted {
            president: 0,
            hitler: 4
        }
    );
    assert_eq!(summary.winning_party, Party::Liberal);

    let end_chan = h.take_channel_plain();
    assert!(end_chan.contains("Alice EXECUTED Eve (HITLER) — LIBERALS WIN!"));
    assert!(end_chan.contains("🏆 Winners: Alice (Liberal), Bob (Liberal), Carol (Liberal)"));
    assert!(!end_chan.contains('`'));

    let reveal_text = summary
        .reveal
        .lines()
        .iter()
        .map(Text::plain)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(reveal_text.contains("🃏 Secret Roles: Alice: Liberal · Bob: Liberal 💀 · Carol: Liberal · Dave: Fascist · Eve: Hitler 💀"));
    assert!(reveal_text.contains("Policy Peek: Carol saw [FASCIST, FASCIST, FASCIST]"));
    assert!(reveal_text.contains("Execution: Alice executed Eve (Hitler) — Liberals win!"));
    assert!(!reveal_text.contains('`'));
}

#[tokio::test(start_paused = true)]
async fn seven_player_blind_hitler_investigate_special_election_double_turn_and_hitler_election_win(
) {
    let deals = [
        ("Alice", Role::Liberal),
        ("Bob", Role::Liberal),
        ("Carol", Role::Liberal),
        ("Dave", Role::Liberal),
        ("Eve", Role::Fascist),
        ("Frank", Role::Fascist),
        ("Grace", Role::Hitler),
    ];
    let deck = [Policy::Fascist; 17];

    let mut h = ShTestHarness::new(&deals, &deck, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;

    // 1. In 7–10p, Hitler does NOT know the Fascists, while Eve & Frank see Hitler + each other:
    let eve_dm = h.take_dm_plain("Eve");
    let frank_dm = h.take_dm_plain("Frank");
    let grace_dm = h.take_dm_plain("Grace");
    assert!(eve_dm.contains("Hitler: Grace · Fellow Fascist(s): Frank"));
    assert!(frank_dm.contains("Hitler: Grace · Fellow Fascist(s): Eve"));
    assert!(grace_dm.contains("You do NOT know who the Fascists are"));
    assert!(!grace_dm.contains("Eve"));
    assert!(!grace_dm.contains("Frank"));

    // Public `!role` in channel never leaks secret role; DM `!role` replies privately:
    let _ = h.take_channel_plain();
    h.chat("Grace", "!role").await;
    let pub_role = h.take_channel_plain();
    assert!(pub_role.contains("7-player setup: 4 Liberals, 2 Fascists, 1 Hitler"));
    assert!(!pub_role.contains("Your Secret Role"));

    h.dm("Grace", "!role").await;
    assert!(h
        .take_dm_plain("Grace")
        .contains("Your Secret Role: HITLER"));

    let all_ja = [
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
        ("Frank", "ja"),
        ("Grace", "ja"),
    ];

    // Round 1 (Alice + Bob): F #1 (no power in 7–8p):
    h.chat("Alice", "!chancellor Bob").await;
    h.vote_all(&all_ja).await;
    h.dm("Alice", "f").await;
    h.dm("Bob", "f").await;

    // Round 2 (Bob + Carol): Alice & Bob were term-limited as Chan; F #2 triggers Investigate Loyalty!
    // Bob inspects Grace (Hitler) -> sees Party Membership FASCIST (NOT "Hitler"!):
    h.chat("Bob", "!chancellor Alice").await;
    assert!(h
        .take_channel_plain()
        .contains("term-limited (last elected President)"));
    h.chat("Bob", "!chancellor Carol").await;
    h.vote_all(&all_ja).await;
    h.dm("Bob", "f").await;
    h.dm("Carol", "f").await;

    let _ = h.take_dm_plain("Bob");
    h.chat("Bob", "!investigate Grace").await;
    let bob_inv_dm = h.take_dm_plain("Bob");
    assert!(bob_inv_dm.contains("Grace's Party Membership card is FASCIST"));
    assert!(!bob_inv_dm.contains("HITLER"));

    // Round 3 (Carol + Eve): F #3 triggers Call Special Election + Hitler Territory!
    // Carol appoints Dave (immediately to Carol's left!) -> Dave must run in Round 4 (Special) AND Round 5 (Regular)!
    h.chat("Carol", "!chancellor Eve").await;
    h.vote_all(&all_ja).await;
    h.dm("Carol", "f").await;
    h.dm("Eve", "f").await;
    h.chat("Carol", "!elect Dave").await;
    let r4_start_chan = h.take_channel_plain();
    assert!(r4_start_chan.contains("Special Pres: Dave"));

    // Round 4 (Special Election — Dave): Dave nominates Alice; let's reject it to test normal rotation resumption!
    h.chat("Dave", "!chancellor Alice").await;
    h.vote_all(&[
        ("Alice", "nein"),
        ("Bob", "nein"),
        ("Carol", "nein"),
        ("Dave", "nein"),
        ("Eve", "ja"),
        ("Frank", "ja"),
        ("Grace", "ja"),
    ])
    .await;
    let r4_chan = h.take_channel_plain();
    assert!(r4_chan.contains("Election failed (1/3)"));
    assert!(r4_chan.contains("Round 5") && r4_chan.contains("Pres: Dave"));

    // Round 5 (Normal rotation resumes to Carol's left -> Dave runs AGAIN!):
    // Also verify Carol & Eve (the last *elected* government from Round 3, NOT failed Dave & Alice!) are still term-limited:
    h.chat("Dave", "!chancellor Carol").await;
    let tl_reject = h.take_channel_plain();
    assert!(tl_reject.contains("term-limited (last elected President)"));

    // Dave nominates Grace (Hitler!) in Hitler Territory -> voting Ja wins immediately for Fascists!
    h.chat("Dave", "!chancellor Grace").await;
    h.vote_all(&all_ja).await;

    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(
        summary.win_reason,
        WinReason::HitlerElectedChancellor { chancellor: 6 }
    );
    assert_eq!(summary.winning_party, Party::Fascist);
    let win_chan = h.take_channel_plain();
    assert!(win_chan.contains("Grace (HITLER) WAS ELECTED CHANCELLOR IN HITLER TERRITORY"));
    assert!(win_chan.contains("🏆 Winners: Eve (Fascist), Frank (Fascist), Grace (Fascist)"));
}

#[tokio::test(start_paused = true)]
async fn ballot_updates_session_silence_veto_refusal_and_veto_chaos_into_liberal_win() {
    let deals = [
        ("Alice", Role::Liberal),
        ("Bob", Role::Liberal),
        ("Carol", Role::Liberal),
        ("Dave", Role::Fascist),
        ("Eve", Role::Hitler),
    ];
    // Setup deck so we quickly reach 4 Liberal + 5 Fascist policies, then test Veto refusal + Veto Chaos!
    let deck = [
        // R1 (Alice+Bob): [L, L, L] -> L #1
        Policy::Liberal,
        Policy::Liberal,
        Policy::Liberal,
        // R2 (Bob+Carol): [L, L, L] -> L #2
        Policy::Liberal,
        Policy::Liberal,
        Policy::Liberal,
        // R3 (Carol+Dave): [L, L, L] -> L #3
        Policy::Liberal,
        Policy::Liberal,
        Policy::Liberal,
        // R4 (Dave+Eve): [F, F, F] -> F #1
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // R5 (Eve+Alice): [F, F, F] -> F #2
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // Remaining 2 -> reshuffles! To avoid shuffle randomness, let's pass a 30-card custom deck!
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // R7: F #4 (+ kill)
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // R8: F #5 (+ kill + Veto unlocked!)
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        // R9 (Veto refused -> enacts L #4):
        Policy::Liberal,
        Policy::Fascist,
        Policy::Fascist,
        // R12 (Veto consented at Tracker 2/3 -> Chaos draws top L #5!):
        Policy::Fascist,
        Policy::Fascist,
        Policy::Fascist,
        Policy::Liberal,
        Policy::Liberal,
        Policy::Liberal,
    ];

    let mut h = ShTestHarness::new(&deals, &deck, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;

    let all_ja = [
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ];

    // R1: L #1 — also test public `!vote` channel reminder and DM ballot changing before final voter!
    h.chat("Alice", "!chancellor Bob").await;
    let _ = h.take_channel_plain();
    h.chat("Alice", "!vote ja").await;
    assert!(h
        .take_channel_plain()
        .contains("Ballots are secret — vote via /msg GM ja or nein!"));

    h.dm("Alice", "ja").await;
    h.dm("Alice", "nein").await;
    h.dm("Alice", "ja").await;
    let alice_ballot_dms = h.take_dm_plain("Alice");
    assert!(alice_ballot_dms.contains("Ballot updated: NEIN!"));
    assert!(alice_ballot_dms.contains("Ballot updated: JA!"));

    h.vote_all(&[
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Dave", "ja"),
        ("Eve", "ja"),
    ])
    .await;

    // Verify President & Chancellor were devoiced (`-v`) on entering Legislative Session:
    let mut saw_devoice_session = false;
    while let Some(cmd) = h.srv.try_recv_sent() {
        if let Command::Raw(verb, args) = cmd {
            if verb == "MODE" && args.get(1).is_some_and(|m| m.starts_with("-v")) {
                saw_devoice_session = true;
            }
        }
    }
    assert!(
        saw_devoice_session,
        "expected President & Chancellor to be devoiced during Legislative Session"
    );

    // Verify anti-inversion hints on legislative DMs:
    let _ = h.take_dm_plain("Alice");
    h.dm("Alice", "enact liberal").await;
    assert!(h.take_dm_plain("Alice").contains("DISCARD"));
    h.dm("Alice", "l").await;

    let _ = h.take_dm_plain("Bob");
    h.dm("Bob", "discard liberal").await;
    assert!(h.take_dm_plain("Bob").contains("ENACT"));
    h.dm("Bob", "l").await;

    // R2..R8 to reach Lib 3/5, Fas 5/6:
    // R2 (Bob+Carol): L #2
    h.chat("Bob", "!chancellor Carol").await;
    h.vote_all(&all_ja).await;
    h.dm("Bob", "l").await;
    h.dm("Carol", "l").await;

    // R3 (Carol+Dave): L #3
    h.chat("Carol", "!chancellor Dave").await;
    h.vote_all(&all_ja).await;
    h.dm("Carol", "l").await;
    h.dm("Dave", "l").await;

    // R4 (Dave+Alice): F #1
    h.chat("Dave", "!chancellor Alice").await;
    h.vote_all(&all_ja).await;
    h.dm("Dave", "f").await;
    h.dm("Alice", "f").await;

    // R5 (Eve+Bob): F #2
    h.chat("Eve", "!chancellor Bob").await;
    h.vote_all(&all_ja).await;
    h.dm("Eve", "f").await;
    h.dm("Bob", "f").await;

    // R6 (Alice+Carol): F #3 (Policy Peek)
    h.chat("Alice", "!chancellor Carol").await;
    h.vote_all(&all_ja).await;
    h.dm("Alice", "f").await;
    h.dm("Carol", "f").await;

    // R7 (Bob+Alice): F #4 -> Bob executes Dave (Fascist, leaving 4 alive: Alice, Bob, Carol, Eve)
    h.chat("Bob", "!chancellor Alice").await;
    h.vote_all(&all_ja).await;
    h.dm("Bob", "f").await;
    h.dm("Alice", "f").await;
    h.chat("Bob", "!kill Dave").await;

    let alive4_ja = [
        ("Alice", "ja"),
        ("Bob", "ja"),
        ("Carol", "ja"),
        ("Eve", "ja"),
    ];

    // R8 (Carol+Bob): F #5 -> Veto unlocked! Carol executes Bob (leaving 3 alive: Alice, Carol, Eve)
    h.chat("Carol", "!chancellor Bob").await;
    h.vote_all(&alive4_ja).await;
    h.dm("Carol", "f").await;
    h.dm("Bob", "f").await;
    h.chat("Carol", "!kill Bob").await;

    let alive3_ja = [("Alice", "ja"), ("Carol", "ja"), ("Eve", "ja")];

    // R9 (Eve+Alice): Draws [L, F, F]. Eve discards F -> passes [L, F] to Alice.
    // Alice proposes VETO -> Eve REFUSES (`nein`) -> Alice cannot veto again and enacts LIBERAL (L #4)!
    h.chat("Eve", "!chancellor Alice").await;
    h.vote_all(&alive3_ja).await;
    h.dm("Eve", "f").await;
    h.dm("Alice", "veto").await;
    h.dm("Eve", "nein").await;

    let _ = h.take_dm_plain("Alice");
    h.dm("Alice", "veto").await;
    assert!(h
        .take_dm_plain("Alice")
        .contains("Veto Power is not available"));
    h.dm("Alice", "liberal").await;

    let r9_chan = h.take_channel_plain();
    assert!(r9_chan.contains("confirmed NOT Hitler"));
    assert!(r9_chan.contains("President Eve refuses the Veto!"));
    assert!(r9_chan.contains("enacts a LIBERAL policy"));

    // R10 & R11: Fail two elections in a row -> Tracker 2/3!
    let alive3_nein = [("Alice", "nein"), ("Carol", "nein"), ("Eve", "nein")];
    // R10 (Alice runs; Alice was last Chan, so Carol is eligible!):
    h.chat("Alice", "!chancellor Carol").await;
    h.vote_all(&alive3_nein).await;
    // R11 (Carol runs; Alice was last elected Chan and STILL term-limited across failed R10 -> Eve is eligible? Don't elect Eve! Wait: fails anyway!):
    h.chat("Carol", "!chancellor Eve").await;
    h.vote_all(&alive3_nein).await;
    assert!(h.take_channel_plain().contains("Election failed (2/3)"));

    // R12 (Eve runs, nominates Carol -> elected! Tracker is STILL 2/3, so consented Veto pushes it to 3/3 -> CHAOS enacts top L #5 -> Liberals win!):
    h.chat("Eve", "!chancellor Carol").await;
    h.vote_all(&alive3_ja).await;
    h.dm("Eve", "f").await;
    h.dm("Carol", "veto").await;
    h.dm("Eve", "ja").await;

    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(summary.win_reason, WinReason::FiveLiberalPolicies);
    assert_eq!(summary.final_board.lib_policies, 5);
    let final_chan = h.take_channel_plain();
    assert!(final_chan.contains("Election Tracker reaches 3/3"));
    assert!(final_chan.contains("CHAOS!"));
    assert!(final_chan.contains("5 LIBERAL POLICIES ENACTED — LIBERALS WIN!"));
}

#[tokio::test(start_paused = true)]
async fn nine_player_double_investigate_blocks_repeat_target_and_reaches_six_fascist_win() {
    let deals = [
        ("P1", Role::Liberal),
        ("P2", Role::Liberal),
        ("P3", Role::Liberal),
        ("P4", Role::Liberal),
        ("P5", Role::Liberal),
        ("P6", Role::Fascist),
        ("P7", Role::Fascist),
        ("P8", Role::Fascist),
        ("P9", Role::Hitler),
    ];
    let deck = [Policy::Fascist; 24];

    let mut h = ShTestHarness::new(&deals, &deck, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;

    let all_ja = [
        ("P1", "ja"),
        ("P2", "ja"),
        ("P3", "ja"),
        ("P4", "ja"),
        ("P5", "ja"),
        ("P6", "ja"),
        ("P7", "ja"),
        ("P8", "ja"),
        ("P9", "ja"),
    ];

    // R1 (P1 + P2): F #1 in 9–10p triggers Investigate Loyalty #1 -> P1 inspects P3:
    h.chat("P1", "!chancellor P2").await;
    h.vote_all(&all_ja).await;
    h.dm("P1", "f").await;
    h.dm("P2", "f").await;
    h.chat("P1", "!investigate P3").await;

    // R2 (P2 + P3): F #2 in 9–10p triggers Investigate Loyalty #2 -> P2 CANNOT inspect P3 again!
    h.chat("P2", "!chancellor P3").await;
    h.vote_all(&all_ja).await;
    h.dm("P2", "f").await;
    h.dm("P3", "f").await;
    let _ = h.take_channel_plain();
    h.chat("P2", "!investigate P3").await;
    assert!(h
        .take_channel_plain()
        .contains("already been investigated in this game"));
    h.chat("P2", "!investigate P4").await;

    // R3 (P3 + P4): F #3 -> Special Election -> P3 picks P5:
    h.chat("P3", "!chancellor P4").await;
    h.vote_all(&all_ja).await;
    h.dm("P3", "f").await;
    h.dm("P4", "f").await;
    h.chat("P3", "!elect P5").await;

    // R4 (P5 + P6): F #4 -> Execution -> P5 kills P1:
    h.chat("P5", "!chancellor P6").await;
    h.vote_all(&all_ja).await;
    h.dm("P5", "f").await;
    h.dm("P6", "f").await;
    h.chat("P5", "!kill P1").await;

    let alive8_ja = [
        ("P2", "ja"),
        ("P3", "ja"),
        ("P4", "ja"),
        ("P5", "ja"),
        ("P6", "ja"),
        ("P7", "ja"),
        ("P8", "ja"),
        ("P9", "ja"),
    ];

    // R5 (P4, resuming to P3's left!): F #5 -> P4 kills P2:
    h.chat("P4", "!chancellor P7").await;
    h.vote_all(&alive8_ja).await;
    h.dm("P4", "f").await;
    h.dm("P7", "f").await;
    h.chat("P4", "!kill P2").await;

    let alive7_ja = [
        ("P3", "ja"),
        ("P4", "ja"),
        ("P5", "ja"),
        ("P6", "ja"),
        ("P7", "ja"),
        ("P8", "ja"),
        ("P9", "ja"),
    ];

    // R6 (P5 + P8): F #6 -> Immediate Fascist victory!
    h.chat("P5", "!chancellor P8").await;
    h.vote_all(&alive7_ja).await;
    h.dm("P5", "f").await;
    h.dm("P8", "f").await;

    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(summary.win_reason, WinReason::SixFascistPolicies);
    assert_eq!(summary.winning_party, Party::Fascist);
}

#[tokio::test(start_paused = true)]
async fn lobby_recruitment_host_handoff_queries_departure_and_abort_moderation_restore() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#sh", &["Alice", "Bob", "Carol", "Dave", "Eve", "Frank"]);
    srv.send_line(":Alice!~Alice@test.host TOPIC #sh :Welcome to #sh!\r\n");
    srv.settle().await;

    let room = bot.room("#sh").unwrap();
    let cfg = GameConfig::for_tests();

    // 1. Open recruitment lobby, verify host handoff, catalog queries, `< 5` `!go` rejection, and `!players`:
    let recruit_task = tokio::spawn({
        let room = room.clone();
        let host = bot.user("Alice").unwrap();
        let cfg = cfg.clone();
        async move { recruit_players(&room, host, &cfg).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#sh");

    srv.chat("Alice", "#sh", "!rules");
    srv.chat("Alice", "#sh", "!tracks");
    srv.chat("Alice", "#sh", "!tracks 7p");
    srv.chat("Alice", "#sh", "!tracks 3p");
    srv.chat("Bob", "#sh", "!join");
    srv.chat("Alice", "#sh", "!leave");
    srv.chat("Bob", "#sh", "!go");
    srv.settle().await;
    let out = Text::parse(&srv.take_messages_to("#sh").join("\n")).plain();
    assert!(out.contains("Secret Hitler Rules"));
    assert!(out.contains("7-Player Secret Hitler — 4 Liberals, 2 Fascists, 1 Hitler"));
    assert!(out.contains("Invalid player count '3p'"));
    assert!(out.contains("Alice left — Bob is now lobby host!"));
    assert!(out.contains("Need at least 5 players"));
    assert!(!out.contains('`'));

    srv.chat("Alice", "#sh", "!join");
    srv.chat("Carol", "#sh", "!join");
    srv.chat("Dave", "#sh", "!join");
    srv.chat("Eve", "#sh", "!join");
    srv.chat("Bob", "#sh", "!players");
    srv.chat("Bob", "#sh", "!help");
    srv.chat("Bob", "#sh", "!go");
    srv.settle().await;
    let players = recruit_task
        .await
        .unwrap()
        .unwrap()
        .expect("5 players recruited");
    assert_eq!(players.len(), 5);
    let lobby_info = Text::parse(&srv.take_messages_to("#sh").join("\n")).plain();
    assert!(lobby_info.contains("Lobby (5/10):"));
    assert!(lobby_info.contains("Open Lobby"));
    assert!(!lobby_info.contains('`'));

    // Also verify `handle_catalog_query` ignores non-Secret-Hitler commands:
    let mut inbox = room.inbox();
    srv.chat("Alice", "#sh", "!roles");
    srv.settle().await;
    let msg = inbox.next_chat().await.unwrap();
    assert!(!handle_catalog_query(&room, &msg).unwrap());

    // 2. Start a game via `run_single_game`, query live `!board`/`!players`/`!eligible`/`!time`/`!help`,
    //    then verify living player parting ends game with `Error::UserLeft` and restores `-m` + original topic:
    let game_task = tokio::spawn({
        let bot = bot.clone();
        let room = room.clone();
        let host = bot.user("Alice").unwrap();
        let cfg = cfg.clone();
        async move { run_single_game(&bot, &room, host, &cfg).await }
    });
    srv.settle().await;
    srv.chat("Bob", "#sh", "!join");
    srv.chat("Carol", "#sh", "!join");
    srv.chat("Dave", "#sh", "!join");
    srv.chat("Eve", "#sh", "!join");
    srv.chat("Alice", "#sh", "!go");
    srv.settle().await;
    let _ = srv.take_messages_to("#sh");

    srv.chat("Frank", "#sh", "!board");
    srv.chat("Frank", "#sh", "!players");
    srv.chat("Frank", "#sh", "!eligible");
    srv.chat("Frank", "#sh", "!time");
    srv.chat("Frank", "#sh", "!help");
    srv.settle().await;
    let active_info = Text::parse(&srv.take_messages_to("#sh").join("\n")).plain();
    assert!(active_info.contains("Round 1"));
    assert!(active_info.contains("Eligible Chancellors"));
    assert!(active_info.contains("Table order (5/5 alive)"));
    assert!(active_info.contains("remaining in Nomination"));
    assert!(active_info.contains("Active Game"));
    assert!(!active_info.contains('`'));

    while srv.try_recv_sent().is_some() {}
    srv.part_user("Carol", "#sh", Some("gtg"));
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
            Command::TOPIC(_, Some(t)) if t == "Welcome to #sh!" => {
                saw_topic = true;
            }
            _ => {}
        }
    }
    assert!(saw_unmute, "expected MODE -m on living player departure");
    assert!(saw_topic, "expected original topic restored on departure");

    // 3. Rejoin Carol, start another game, and cancel mid-game via `.until(...)` (as `bot` does on `!abort`):
    srv.join_user("Carol", "#sh");
    srv.settle().await;
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let abort_task = tokio::spawn({
        let bot = bot.clone();
        let room = room.clone();
        let host = bot.user("Alice").unwrap();
        async move {
            run_single_game(&bot, &room, host, &cfg)
                .until(async {
                    let _ = cancel_rx.await;
                })
                .await
        }
    });
    srv.settle().await;
    srv.chat("Bob", "#sh", "!join");
    srv.chat("Carol", "#sh", "!join");
    srv.chat("Dave", "#sh", "!join");
    srv.chat("Eve", "#sh", "!join");
    srv.chat("Alice", "#sh", "!go");
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

#[tokio::test(start_paused = true)]
async fn afk_players_timing_out_auto_advance_to_chaos_completion_without_deadlocking() {
    let deals = [
        ("Alice", Role::Liberal),
        ("Bob", Role::Liberal),
        ("Carol", Role::Liberal),
        ("Dave", Role::Fascist),
        ("Eve", Role::Hitler),
    ];
    let deck = [Policy::Liberal; 17];
    let h = ShTestHarness::new(&deals, &deck, 0).await;
    let game_task = h.spawn_game();
    h.srv.settle().await;

    // Nobody sends a single message! Each round: 15s nominate timeout + 10s vote timeout = 25s -> Nein!
    // 3 failed rounds (75s) -> 1 Liberal Chaos policy! 5 Chaos policies (375s) -> Liberals win!
    for _ in 0..80 {
        if game_task.is_finished() {
            break;
        }
        h.srv.advance(Duration::from_secs(5)).await;
    }
    let summary = game_task.await.unwrap().unwrap();
    assert_eq!(summary.win_reason, WinReason::FiveLiberalPolicies);
}

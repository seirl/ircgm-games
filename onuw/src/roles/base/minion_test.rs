use crate::roles::base::{minion::MINION, tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::dream_wolf::DREAM_WOLF;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn minion_sees_both_waking_and_dream_wolves() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MINION),
            ("Bob", &WEREWOLF),
            ("Carol", &DREAM_WOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Carol sleeps as Dream Wolf with thumb out, so waking wolf Bob sees Carol and does NOT get Lone Wolf center peek:
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Bob").contains("Dream Wolf Carol"));
    assert!(!report.dms_to("Bob").contains("LONE WOLF"));
    assert!(report
        .dms_to("Alice")
        .contains("The Werewolves are Bob, Carol"));
}

#[tokio::test(start_paused = true)]
async fn solo_minion_with_zero_wolves_follows_official_onuw_victory_rules() {
    // Case 1: Nobody dies (1-1-1 circle vote) -> Village wins, Minion loses!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MINION),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    let report = h.finish_night(night).await;
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);

    // Case 2: Villager Bob dies while Minion Alice survives -> Minion wins!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &MINION),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    let report2 = h2.finish_night(night2).await;
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Alice"]);

    // Case 3: Tanner dies while Minion survives -> ONLY Tanner wins (blocks Evil)!
    let mut h3 = OnuwTestHarness::new(
        &[("Alice", &MINION), ("Bob", &TANNER), ("Carol", &VILLAGER)],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night3 = h3.spawn_night();
    let report3 = h3.finish_night(night3).await;
    let dawn3 = h3
        .resolve_voting(
            report3,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Bob"]);

    // Case 4: Both Minion and Squire in play with 0 Werewolves; ONLY Squire dies -> surviving Minion does NOT win!
    use crate::roles::vampire::squire::SQUIRE;
    let mut h4 = OnuwTestHarness::new(
        &[("Alice", &MINION), ("Bob", &SQUIRE), ("Carol", &VILLAGER)],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let n4 = h4.spawn_night();
    let report4 = h4.finish_night(n4).await;
    let dawn4 = h4
        .resolve_voting(
            report4,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert!(dawn4.winner_nicks(&h4.roster).is_empty());
}

#[tokio::test(start_paused = true)]
async fn doppelganger_minion_acts_immediately_at_wake_1_respects_fear_and_does_not_wake_at_3() {
    use crate::roles::alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE};
    use crate::roles::base::doppelganger::DOPPELGANGER;
    use crate::roles::vampire::count::COUNT;
    use ircgm::prelude::Text;

    // 1. Night-only game: Doppelgänger-Minion sees Werewolves at #1 and does not wake with Minion at #3:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MINION),
            ("Carol", &WEREWOLF),
            ("Dave", &ORACLE),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 3,
    })));
    let night = h.spawn_night();
    h.reply_dm("Dave", "3").await;
    h.reply_dm("Alice", "Bob").await; // Doppelgänger copies Minion
    h.reply_dm("Carol", "1").await; // Lone Werewolf peeks Center #1
    let report = h.finish_night(night).await;

    let alice_plain = Text::parse(&report.dms_to("Alice")).plain();
    let bob_plain = Text::parse(&report.dms_to("Bob")).plain();
    let dave_watch = Text::parse(&report.dms_to("Dave")).plain();
    assert!(alice_plain.contains("Doppelgänger-Minion: The Werewolves are Carol"));
    assert!(bob_plain.contains("Minion: The Werewolves are Carol"));
    assert!(!bob_plain.contains("Alice"));
    assert!(
        dave_watch.contains("Doppelgänger (copied action): Alice")
            && dave_watch.contains("Minion: Bob")
            && !dave_watch.contains("Minion: Alice, Bob"),
        "got:\n{dave_watch}"
    );

    // 2. Dusk + Night game: Doppelgänger-Minion given Mark::Fear at #-6B is paralyzed at Night #1:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MINION),
            ("Carol", &WEREWOLF),
            ("Dave", &COUNT),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await; // Dusk #-7: copies Minion
    h2.reply_dm("Dave", "Carol").await; // Dusk #-6: Vampire mark
    h2.reply_dm("Dave", "Alice").await; // Dusk #-6B: Mark::Fear on Alice!
    h2.reply_dm("Carol", "1").await;
    let rep2 = h2.finish_night(n2).await;
    assert!(!Text::parse(&rep2.dms_to("Alice"))
        .plain()
        .contains("The Werewolves are"));
}

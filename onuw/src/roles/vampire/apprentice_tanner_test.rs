use crate::roles::base::{tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::vampire::apprentice_tanner::APPRENTICE_TANNER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn apprentice_tanner_wins_when_tanner_dies_or_becomes_backup_tanner_when_alone() {
    // 1. Tanner in play: Apprentice Tanner sees Tanner and wins iff Tanner dies (even if AT survives)!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &APPRENTICE_TANNER),
            ("Bob", &TANNER),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "1").await;
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("The Tanner is Bob"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);

    // 2. Tanner in the center: Apprentice Tanner must die to win!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &APPRENTICE_TANNER),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&TANNER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Bob", "2").await;
    let report2 = h2.finish_night(night2).await;
    assert!(report2.dms_to("Alice").contains("NO player holds the Tanner"));

    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Alice"]);

    // 3. Apprentice Tanner + Doppelgänger-Apprentice Tanner wake separately without seeing each other:
    use crate::roles::alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE};
    use crate::roles::base::doppelganger::DOPPELGANGER;
    use ircgm::prelude::Text;

    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &APPRENTICE_TANNER),
            ("Bob", &DOPPELGANGER),
            ("Carol", &TANNER),
            ("Dave", &ORACLE),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 5,
    })));
    let n3 = h3.spawn_night();
    h3.reply_dm("Dave", "5").await;
    h3.reply_dm("Bob", "Alice").await; // Doppelgänger -> Apprentice Tanner
    let rep3 = h3.finish_night(n3).await;

    let alice_dm = Text::parse(&rep3.dms_to("Alice")).plain();
    let bob_dm = Text::parse(&rep3.dms_to("Bob")).plain();
    let dave_watch = Text::parse(&rep3.dms_to("Dave")).plain();
    assert!(alice_dm.contains("Apprentice Tanner: The Tanner is Carol"));
    assert!(!alice_dm.contains("Bob") && !alice_dm.contains("fellow"));
    assert!(bob_dm.contains("Doppelgänger-Apprentice Tanner: The Tanner is Carol"));
    assert!(
        dave_watch.contains("Apprentice Tanner: Alice · Doppelgänger-Apprentice Tanner: Bob"),
        "got:\n{dave_watch}"
    );
}

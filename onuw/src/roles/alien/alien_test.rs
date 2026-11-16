use crate::roles::alien::{
    alien::{ALIEN, SYNTHETIC_ALIEN},
    blob::BLOB,
    cow::COW,
    groob_zerb::{GROOB, ZERB},
    leader::LEADER,
    mortician::MORTICIAN,
    oracle::ORACLE,
    psychic::PSYCHIC,
};
use crate::roles::base::villager::VILLAGER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn oracle_amplified_signals_doubles_psychic_vision_and_cow_and_aliens_spot_each_other() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &COW),
            ("Carol", &ORACLE),
            ("Dave", &PSYCHIC),
            ("Eve", &SYNTHETIC_ALIEN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Oracle (Carol, #-9) tunes frequencies first, then Alien (Alice, #1A) wakes WITH Synthetic Alien (Eve) and scans Carol!
    h.reply_dm("Carol", "amplify").await;
    h.reply_dm("Alice", "scan Carol").await;

    let report = h.finish_night(night).await;
    // Psychic (Dave, seat #4) neighbors are Carol (#3, Oracle) and Eve (#5, Synthetic Alien):
    // Because Oracle amplified signals, Dave reads BOTH neighbors!
    let dave_dms = report.dms_to("Dave");
    assert!(dave_dms.contains("Carol holds Oracle"));
    assert!(dave_dms.contains("Eve holds Synthetic Alien"));
    // Cow (Bob, seat #2) sits next to Alice (#1, Alien) -> hoof tapped, AND waking Aliens see Cow!
    assert!(report.dms_to("Bob").contains("*TAP!*"));
    assert!(report.dms_to("Alice").contains("Bob holds the Cow card"));
    assert!(report.dms_to("Eve").contains("Bob holds the Cow card"));
    assert!(report.dms_to("Eve").contains("Carol holds Oracle"));
    assert!(report.has_chronicle_substr("Alien (Alice) bio-scanned Carol and saw Oracle."));

    // If Synthetic Alien (Eve) dies alongside Alien (Alice) on a 2-2 tie, Synthetic Alien wins ALONE (blocking Village too!):
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Eve")),
                ("Bob", Some("Eve")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Eve"]);
}

#[tokio::test(start_paused = true)]
async fn leader_unanimous_alien_point_steals_victory_even_when_alien_dies() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &ALIEN),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "pass").await;
    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Carol")
        .contains("The Aliens are Alice, Bob"));

    // Even though Carol and Dave tie/kill Alice (2-2), BOTH Aliens (Alice & Bob) pointed at Leader Carol!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);
    assert!(dawn
        .channel_text()
        .contains("All Aliens pointed at the Leader"));
}

#[tokio::test(start_paused = true)]
async fn groob_zerb_blob_mortician_and_tractor_beam_work() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &BLOB),
            ("Eve", &MORTICIAN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Non-aliens are Dave (Blob) and Eve (Mortician); Alien tractor-beams them!
    h.reply_dm("Alice", "beam Dave Eve").await;
    // Eve (waking as Mortician at #13A) peeks her own card and discovers she now holds Blob!
    h.reply_dm("Eve", "self").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Non-Aliens: Dave, Eve"));
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Mortician");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Blob");
    assert!(report.dms_to("Eve").contains("Your own card is Blob"));
    assert!(report.has_chronicle_substr(
        "Alien (Alice) tractor-beamed a swap between Dave (Blob) and Eve (Mortician)."
    ));

    // At Dawn, Carol (Zerb) is killed!
    // - Killing Zerb counts as killing an Alien -> regular Alien Alice LOSES!
    // - Bob (Groob): survived and Zerb died -> Groob WINS!
    // - Dave (now Mortician, neighbors Carol & Eve): Carol died & Dave survived -> Mortician WINS!
    // - Eve (now Blob at seat #5, <6 players -> Blob spans {Eve, left=Dave}): both survived -> Blob WINS!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Dave", "Eve"]);
}

#[tokio::test(start_paused = true)]
async fn solo_groob_joins_alien_team_and_leader_wins_when_both_groob_and_zerb_survive() {
    // 1. Solo Groob (Zerb in center) -> Groob is on Team::Alien!
    let mut h1 = OnuwTestHarness::new(
        &[("Alice", &GROOB), ("Bob", &VILLAGER), ("Carol", &VILLAGER)],
        [&ZERB, &VILLAGER, &VILLAGER],
    )
    .await;
    let night1 = h1.spawn_night();
    h1.reply_dm("Alice", "pass").await;
    let report1 = h1.finish_night(night1).await;
    assert!(report1
        .dms_to("Alice")
        .contains("you are on the regular Alien team"));
    let dawn1 = h1
        .resolve_dawn(
            report1,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Bob")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn1.winner_nicks(&h1.roster), vec!["Alice"]);

    // 2. Both Groob & Zerb in play with Leader -> Leader sees both and wins iff neither dies!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &GROOB),
            ("Bob", &ZERB),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Alice", "pass").await;
    let report2 = h2.finish_night(night2).await;
    assert!(report2
        .dms_to("Carol")
        .contains("Groob & Zerb are pointing at each other (Alice, Bob)"));
    let dawn2 = h2
        .resolve_dawn(
            report2,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol"]);
}

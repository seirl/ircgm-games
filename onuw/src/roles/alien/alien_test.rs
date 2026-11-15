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
async fn oracle_amplified_signals_doubles_psychic_vision_and_cow_feels_alien_tap() {
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
    // Alien (Alice) scans Carol; Oracle (Carol) amplifies signals!
    h.reply_dm("Alice", "scan Carol").await;
    h.reply_dm("Carol", "amplify").await;

    let report = h.finish_night(night).await;
    // Psychic (Dave, seat #4) neighbors are Carol (#3, Oracle) and Eve (#5, Synthetic Alien):
    // Because Oracle amplified signals, Dave reads BOTH neighbors!
    let dave_dms = report.dms_to("Dave");
    assert!(dave_dms.contains("Carol holds Oracle"));
    assert!(dave_dms.contains("Eve holds Synthetic Alien"));
    // Cow (Bob, seat #2) sits next to Alice (#1, Alien) -> hoof tapped!
    assert!(report.dms_to("Bob").contains("*TAP!*"));
    assert!(report.has_chronicle_substr("Alien (Alice) bio-scanned Carol and saw Oracle."));
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

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Mortician");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Blob");
    assert!(report.has_chronicle_substr(
        "Alien (Alice) tractor-beamed a swap between Dave (Blob) and Eve (Mortician)."
    ));

    // Blob absorbed seat #4 (Dave) and left neighbor #3 (Carol, Zerb) on deal (<6 players).
    // At Dawn, Carol (Zerb) is killed!
    // - Bob (Groob): Zerb died -> Groob WINS!
    // - Dave (now Mortician, neighbors Carol & Eve): Carol died & Dave survived -> Mortician WINS!
    // - Eve (now Blob): absorbed Carol died -> Blob LOSES!
    // - Alice (Alien, primary threat) survived -> Evil Team (Alice) ALSO wins!
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
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Dave"]);
}

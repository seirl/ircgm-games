use ircgm::prelude::Text;

use crate::roles::alien::nostradamus::NOSTRADAMUS;
use crate::roles::base::{tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn nostradamus_stops_on_non_village_announces_team_and_must_survive_to_win() {
    // 1. Nostradamus sees Villager then Tanner -> stops, announces Tanner, and co-wins when Tanner dies & Nostradamus lives!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &VILLAGER),
            ("Carol", &TANNER),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Dave", "1").await;
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;
    let report = h.finish_night(night).await;

    assert!(report.channel_lines.iter().any(|l| {
        let plain = Text::parse(l).plain().to_string();
        plain.contains("Nostradamus Prophecy:") && plain.contains("Tanner")
    }));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Carol"]);

    // 2. Nostradamus joins Werewolf team on peek #1 (stopping immediately!), then dies while Werewolf survives:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Bob", "1").await;
    h2.reply_dm("Alice", "Bob").await;
    let report2 = h2.finish_night(night2).await;

    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Bob"]);
}

use crate::roles::base::{hunter::HUNTER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn killed_hunter_takes_down_their_vote_target() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &HUNTER),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn.dead_nicks(&h.roster).contains("Alice"));
    assert!(dawn.dead_nicks(&h.roster).contains("Bob"));
    assert!(dawn.channel_text().contains("HUNTER SHOT!"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Carol"]);
}

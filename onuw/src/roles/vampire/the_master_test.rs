use crate::roles::base::villager::VILLAGER;
use crate::roles::vampire::{the_master::THE_MASTER, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn the_master_cannot_die_when_supported_by_another_vampires_vote() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &THE_MASTER),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Bob").await;
    h.send_dm("Bob", "Carol").await;
    let report = h.finish_night(night).await;

    // Alice (The Master) gets 3 votes, BUT Vampire Bob votes for Alice -> Alice survives and Vampires win!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn.dead_nicks(&h.roster).is_empty());
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Carol"]);
}

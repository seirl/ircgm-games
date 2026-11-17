use crate::roles::base::villager::VILLAGER;
use crate::roles::super_villains::{henchman::HENCHMAN, temptress::TEMPTRESS};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn henchman_recognizes_fellow_super_villains_and_wins_when_villains_survive() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &HENCHMAN),
            ("Bob", &TEMPTRESS),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "Carol").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Alice").contains("Fellow Super Villains: Bob"));
    assert!(report
        .dms_to("Alice")
        .contains("planted the extra Villain card onto Carol"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Carol"]);
}

use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER, windy_wendy::WINDY_WENDY,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn windy_wendy_converts_to_super_villain_when_villain_votes_for_her() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WINDY_WENDY),
            ("Bob", &HENCHMAN),
            ("Carol", &INNOCENT_BYSTANDER),
            ("Dave", &INNOCENT_BYSTANDER),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;

    let night = h.spawn_night();
    let report = h.finish_night(night).await;

    // Bob (Henchman #7) votes Alice (Windy Wendy) -> Alice turns into a Super Villain and wins with Bob when Carol dies!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);
}

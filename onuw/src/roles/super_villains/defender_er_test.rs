use crate::roles::super_villains::{
    defender_er::DEFENDER_ER, henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn defender_er_protects_top_vote_getter_so_runner_up_is_caught() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DEFENDER_ER),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &HENCHMAN),
            ("Dave", &INNOCENT_BYSTANDER),
            ("Eve", &INNOCENT_BYSTANDER),
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

    // Bob gets 3 votes (Alice=Defender-er, Carol, Eve), Carol (Henchman) gets 2 votes (Bob, Dave):
    // Defender-er Alice pointed at top vote-getter Bob -> Bob cannot be caught, so Carol (2 votes) is caught!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
                ("Eve", Some("Bob")),
            ],
        )
        .await;
    let dead = dawn.dead_nicks(&h.roster);
    assert!(!dead.contains("Bob"));
    assert!(dead.contains("Carol"));
    assert_eq!(
        dawn.winner_nicks(&h.roster),
        vec!["Alice", "Bob", "Dave", "Eve"]
    );
}

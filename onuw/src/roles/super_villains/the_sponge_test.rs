use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER, the_sponge::THE_SPONGE,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn the_sponge_absorbs_votes_and_deflects_execution_to_runner_up() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &THE_SPONGE),
            ("Bob", &HENCHMAN),
            ("Carol", &INNOCENT_BYSTANDER),
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

    // Alice (The Sponge) gets 3 votes, Bob (Henchman #7) gets 2 votes -> Alice survives and Bob dies!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    let dead = dawn.dead_nicks(&h.roster);
    assert!(!dead.contains("Alice"));
    assert!(dead.contains("Bob"));
    assert_eq!(
        dawn.winner_nicks(&h.roster),
        vec!["Alice", "Carol", "Dave", "Eve"]
    );
}

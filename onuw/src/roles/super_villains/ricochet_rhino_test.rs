use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER, ricochet_rhino::RICOCHET_RHINO,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn ricochet_rhino_redirects_lethal_vote_onto_pointed_target_even_with_one_vote() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &RICOCHET_RHINO),
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

    // Alice (Ricochet Rhino) gets 3 votes and points at Bob (Henchman #7, 1 vote) -> Bob is caught instead!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    let dead = dawn.dead_nicks(&h.roster);
    assert!(!dead.contains("Alice"));
    assert!(dead.contains("Bob"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Carol", "Dave"]);
}

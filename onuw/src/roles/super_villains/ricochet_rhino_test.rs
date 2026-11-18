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

#[tokio::test(start_paused = true)]
async fn ricochet_rhino_three_seat_lasso_loop_spares_everyone_without_duplicate_spam() {
    // Alice (Rhino, 3 votes) -> Bob (Rhino, 2 votes) <-> Carol (Rhino, 1 vote):
    // closed cycle entered after Alice -> nobody dies, and the chain announces only 3 bounces!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &RICOCHET_RHINO),
            ("Bob", &RICOCHET_RHINO),
            ("Carol", &RICOCHET_RHINO),
            ("Dave", &INNOCENT_BYSTANDER),
            ("Eve", &INNOCENT_BYSTANDER),
            ("Frank", &HENCHMAN),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let night = h.spawn_night();
    let rep = h.finish_night(night).await;
    let dawn = h
        .resolve_voting(
            rep,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
                ("Frank", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn.dead_nicks(&h.roster).is_empty());
    let bounce_count = dawn
        .channel_lines
        .iter()
        .filter(|l| l.contains("ricocheted"))
        .count();
    assert_eq!(bounce_count, 3);
}

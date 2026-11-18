use crate::roles::base::hunter::HUNTER;
use crate::roles::super_villains::{
    defender_er::DEFENDER_ER, henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER,
    ricochet_rhino::RICOCHET_RHINO, the_sponge::THE_SPONGE,
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

#[tokio::test(start_paused = true)]
async fn defender_er_protects_runner_up_after_sponge_ricochet_bounce_and_hunter_shot() {
    // 1. Sponge absorbs 3 votes while Defender-er protects 2-vote runner-up Bob -> nobody dies!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DEFENDER_ER),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &THE_SPONGE),
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
    let n1 = h.spawn_night();
    let rep = h.finish_night(n1).await;
    let dawn = h
        .resolve_voting(
            rep,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
                ("Eve", Some("Carol")),
            ],
        )
        .await;
    assert!(dawn.dead_nicks(&h.roster).is_empty());

    // 2. Ricochet Rhino (Carol, 3 votes) bounces onto Bob (1 vote), who is defended by Alice -> Bob survives!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DEFENDER_ER),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &RICOCHET_RHINO),
            ("Dave", &HENCHMAN),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let n2 = h2.spawn_night();
    let rep2 = h2.finish_night(n2).await;
    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert!(dawn2.dead_nicks(&h2.roster).is_empty());

    // 3. Dying Hunter (Carol, 3 votes) shoots Bob (1 vote), who is defended by Alice -> Bob survives!
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &DEFENDER_ER),
            ("Bob", &HENCHMAN),
            ("Carol", &HUNTER),
            ("Dave", &INNOCENT_BYSTANDER),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let n3 = h3.spawn_night();
    let rep3 = h3.finish_night(n3).await;
    let dawn3 = h3
        .resolve_voting(
            rep3,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert!(!dawn3.dead_nicks(&h3.roster).contains("Bob"));
    assert!(dawn3.dead_nicks(&h3.roster).contains("Carol"));
}

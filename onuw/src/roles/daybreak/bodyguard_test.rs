use crate::roles::base::{hunter::HUNTER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::bodyguard::BODYGUARD;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn bodyguard_protects_target_from_execution_and_deflects_to_runner_up() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &BODYGUARD),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;

    // Alice (Bodyguard) points at Carol (3 votes); Bob (Werewolf) has 2 votes -> Carol survives, Bob dies!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Carol")),
            ],
        )
        .await;

    let dead = dawn.dead_nicks(&h.roster);
    assert!(!dead.contains("Carol"));
    assert!(dead.contains("Bob"));
    assert!(dawn
        .channel_text()
        .contains("Alice (Bodyguard) shielded Carol from execution!"));
    assert_eq!(
        dawn.winner_nicks(&h.roster),
        vec!["Alice", "Carol", "Dave", "Eve"]
    );
}

#[tokio::test(start_paused = true)]
async fn bodyguard_also_blocks_hunter_shot_on_protected_target() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &BODYGUARD),
            ("Bob", &HUNTER),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    let report = h.finish_night(night).await;

    // Alice (Bodyguard) shields Carol (2 votes); Bob (Hunter, 2 votes) dies and shoots Carol -> blocked!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;

    let dead = dawn.dead_nicks(&h.roster);
    assert!(dead.contains("Bob"));
    assert!(!dead.contains("Carol"));
}

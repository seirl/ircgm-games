use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::prince::PRINCE;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn prince_cannot_die_and_deflects_execution_to_runner_up() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &PRINCE),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Lone wolf Bob peeks center #1:
    h.reply_dm("Bob", "1").await;
    let report = h.finish_night(night).await;

    // 3 votes on Alice (Prince) and 2 votes on Bob (Werewolf) -> Prince survives, Bob dies!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Carol", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;

    let dead = dawn.dead_nicks(&h.roster);
    assert!(!dead.contains("Alice"));
    assert!(dead.contains("Bob"));
    assert!(dawn
        .channel_text()
        .contains("Alice (Prince) holds royal immunity and cannot be executed!"));
    assert_eq!(
        dawn.winner_nicks(&h.roster),
        vec!["Alice", "Carol", "Dave", "Eve"]
    );
}

use crate::roles::base::{robber::ROBBER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::vampire::squire::SQUIRE;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn squire_checks_swapped_werewolf_card_and_follows_minion_win_rules() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &ROBBER),
            ("Carol", &SQUIRE),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;
    // Robber Bob steals Werewolf Alice's card (so Alice now holds Robber, Bob holds Werewolf):
    h.reply_dm("Bob", "Alice").await;
    // Squire Carol checks Alice's card at #9C and discovers Alice now holds Robber!
    h.reply_dm("Carol", "all").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Carol").contains("Alice: Robber"));

    // Squire helps Bob (new Werewolf) survive while Alice dies -> Bob & Carol win!
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
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);
}

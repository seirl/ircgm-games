use crate::roles::base::{robber::ROBBER, seer::SEER, villager::VILLAGER};
use crate::roles::daybreak::aura_seer::AURA_SEER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn aura_seer_sees_players_who_viewed_or_moved_cards_and_ignores_passers() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &AURA_SEER),
            ("Bob", &SEER),
            ("Carol", &ROBBER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Bob (Seer) views Dave's card; Carol (Robber) passes (does not view or move a card):
    h.reply_dm("Bob", "Dave").await;
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("Players who viewed or moved a card tonight: Bob"));
    assert!(report
        .has_chronicle_substr("Aura Seer (Alice) sensed active auras on: Bob."));
}

#[tokio::test(start_paused = true)]
async fn aura_seer_reports_when_nobody_viewed_or_moved_a_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &AURA_SEER),
            ("Bob", &SEER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("Nobody else viewed or moved any cards tonight!"));
    assert!(report
        .has_chronicle_substr("Aura Seer (Alice) sensed no active auras."));
}

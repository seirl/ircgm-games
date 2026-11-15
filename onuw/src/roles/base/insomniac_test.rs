use crate::roles::base::{
    insomniac::INSOMNIAC, robber::ROBBER, villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn insomniac_sees_post_robber_card_at_end_of_night() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &INSOMNIAC),
            ("Bob", &ROBBER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "Alice").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("Insomniac: You check your card before dawn — it is Robber!"));
    assert!(report.has_chronicle_substr("Insomniac (Alice) checked their card and saw Robber."));
}

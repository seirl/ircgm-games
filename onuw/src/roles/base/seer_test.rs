use crate::roles::base::{mason::MASON, seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn seer_views_player_card_and_rejects_own_seat() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SEER),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &MASON, &MASON],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    // Reject numeric seat ID `"2"` and self-peek `"Alice"`, then peek `"Bob"`:
    h.reply_dm("Alice", "2").await;
    h.send_dm("Alice", "Alice").await;
    h.send_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("Unknown player nickname."));
    assert!(report
        .dms_to("Alice")
        .contains("You cannot look at your own card!"));
    assert!(report.dms_to("Alice").contains("Bob's card is: Werewolf"));
    assert!(report.has_chronicle_substr("Seer (Alice) viewed Bob's card (Werewolf)."));
}

#[tokio::test(start_paused = true)]
async fn seer_views_two_center_cards() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SEER),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &MASON, &MASON],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "center 1 3").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("Center #1 is Werewolf, Center #3 is Mason."));
    assert!(report.has_chronicle_substr(
        "Seer (Alice) viewed Center #1 (Werewolf) & Center #3 (Mason)."
    ));
}

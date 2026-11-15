use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::witch::WITCH;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn witch_peeks_center_and_swaps_it_onto_player_with_chronicle() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WITCH),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &WEREWOLF, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "2").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Werewolf");
    assert_eq!(report.center_role(2), "Villager");
    assert!(report.has_chronicle_substr(
        "Witch (Alice) peeked Center #2 (Werewolf) and swapped it onto Bob (moving Villager into Center #2)."
    ));
}

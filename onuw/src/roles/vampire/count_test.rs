use crate::marks::Mark;
use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::vampire::count::COUNT;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn count_mark_of_fear_paralyzes_night_seer_completely() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &COUNT),
            ("Bob", &SEER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Fear);
    assert!(report
        .dms_to("Bob")
        .contains("too terrified to wake up tonight"));
    assert!(!report.dms_to("Bob").contains("🔮 Seer:"));
}

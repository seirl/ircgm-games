use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::super_villains::rapscallion::RAPSCALLION;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn rapscallion_peeks_one_center_card_as_super_villain() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &RAPSCALLION),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&SEER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #1 is Seer"));
}

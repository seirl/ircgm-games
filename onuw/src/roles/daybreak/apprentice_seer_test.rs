use crate::roles::base::{seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::apprentice_seer::APPRENTICE_SEER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn apprentice_seer_views_single_center_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &APPRENTICE_SEER),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&SEER, &WEREWOLF, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #1 is: Seer"));
    assert!(report.has_chronicle_substr("Apprentice Seer (Alice) viewed Center #1 (Seer)."));
}

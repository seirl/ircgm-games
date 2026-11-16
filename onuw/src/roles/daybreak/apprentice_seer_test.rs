use std::time::Duration;

use crate::roles::base::{seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::apprentice_seer::APPRENTICE_SEER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn apprentice_seer_acknowledges_choice_immediately_and_views_center_card() {
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
    let _ = h.wait_for_prompt_dm("Alice").await;
    h.advance_time(Duration::from_secs(1)).await;
    h.send_dm("Alice", "1").await;

    let ack = h.wait_for_new_dm("Alice").await;
    assert!(ack.contains("Locked in Center #1"));
    let ack_ts = h.timed_dms["Alice"]
        .iter()
        .find(|(_, m)| m.contains("Locked in Center #1"))
        .unwrap()
        .0;
    assert_eq!(ack_ts, Duration::from_secs(1));

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #1 is: Seer"));
    assert!(report.has_chronicle_substr("Apprentice Seer (Alice) viewed Center #1 (Seer)."));
}

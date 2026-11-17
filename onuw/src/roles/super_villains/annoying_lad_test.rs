use std::time::Duration;

use crate::roles::base::villager::VILLAGER;
use crate::roles::super_villains::annoying_lad::ANNOYING_LAD;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn annoying_lad_pokes_neighbor_rejects_pass_and_auto_pokes_on_timeout() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ANNOYING_LAD),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    let _ = h.wait_for_new_dm("Alice").await;
    h.send_dm("Alice", "pass").await;
    let rej = h.wait_for_new_dm("Alice").await;
    assert!(rej.contains("You MUST pick left or right"));
    h.advance_time(Duration::from_secs(10)).await;

    let report = h.finish_night(night).await;
    let poked_bob = report.dms_to("Bob").contains("Annoying Lad");
    let poked_carol = report.dms_to("Carol").contains("Annoying Lad");
    assert!(poked_bob ^ poked_carol);
}

use crate::roles::base::villager::VILLAGER;
use crate::roles::vampire::thing::THING;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn thing_bumps_immediate_neighbor_and_rejects_non_neighbors() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &THING),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice sits at [1]; immediate neighbors are Dave ([4], left) and Bob ([2], right); Carol ([3]) is across!
    h.reply_dm("Alice", "Carol").await;
    let reject = h.wait_for_new_dm("Alice").await;
    assert!(reject.contains("Pick left, right"));
    h.send_dm("Alice", "right").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Bob").contains("*BUMP!*"));
    assert!(report.dms_to("Bob").contains("left shoulder"));
    assert!(!report.dms_to("Carol").contains("*BUMP!*"));
    assert!(report.has_chronicle_substr("Thing (Alice) bumped neighbor Bob."));
}

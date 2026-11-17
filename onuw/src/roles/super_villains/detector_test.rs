use crate::roles::super_villains::{
    detector::DETECTOR, henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER,
    the_sponge::THE_SPONGE,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn detector_scans_one_player_or_two_center_cards() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DETECTOR),
            ("Bob", &HENCHMAN),
            ("Carol", &INNOCENT_BYSTANDER),
        ],
        [&THE_SPONGE, &INNOCENT_BYSTANDER, &HENCHMAN],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1 3").await;
    let report = h.finish_night(night).await;

    let dms = report.dms_to("Alice");
    assert!(dms.contains("Center #1 is The Sponge"));
    assert!(dms.contains("Center #3 is Henchman #7"));
}

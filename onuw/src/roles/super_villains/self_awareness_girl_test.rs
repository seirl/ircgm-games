use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER,
    self_awareness_girl::SELF_AWARENESS_GIRL, switcheroo::SWITCHEROO,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn self_awareness_girl_sees_swapped_card_at_end_of_night() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SWITCHEROO),
            ("Bob", &HENCHMAN),
            ("Carol", &SELF_AWARENESS_GIRL),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob Carol").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Carol").contains("it is Henchman #7"));
}

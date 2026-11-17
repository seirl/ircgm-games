use crate::roles::super_villains::{
    detector::DETECTOR, henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER,
    switcheroo::SWITCHEROO,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn switcheroo_swaps_two_other_unshielded_players_cards() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SWITCHEROO),
            ("Bob", &HENCHMAN),
            ("Carol", &DETECTOR),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "Bob Carol").await;
    let report = h.finish_night(night).await;

    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Detector");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Henchman #7");
}

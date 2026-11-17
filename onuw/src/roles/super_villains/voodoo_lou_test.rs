use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER, voodoo_lou::VOODOO_LOU,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn voodoo_lou_peeks_center_and_swaps_onto_chosen_player() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VOODOO_LOU),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &INNOCENT_BYSTANDER),
        ],
        [&HENCHMAN, &INNOCENT_BYSTANDER, &INNOCENT_BYSTANDER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Alice").contains("Center #1 is Henchman #7"));
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Henchman #7");
}

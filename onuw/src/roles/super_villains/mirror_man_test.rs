use crate::roles::base::doppelganger::DOPPELGANGER;
use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER, mirror_man::MIRROR_MAN,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn mirror_man_copies_center_role_and_stays_opaque_to_doppelganger() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MIRROR_MAN),
            ("Bob", &DOPPELGANGER),
            ("Carol", &INNOCENT_BYSTANDER),
        ],
        [&HENCHMAN, &INNOCENT_BYSTANDER, &INNOCENT_BYSTANDER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;
    h.reply_dm("Bob", "Alice").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Bob").contains("Mirror Man"));
    assert!(!report.dms_to("Bob").contains("Henchman"));
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Henchman #7");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Henchman #7");
}

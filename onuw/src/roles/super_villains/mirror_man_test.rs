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
    assert!(!report.dms_to("Bob").contains("Copycat"));
    assert!(!report.dms_to("Bob").contains("Henchman"));
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Henchman #7");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Henchman #7");

    // Copycat viewing Mirror Man in Center #1 adopts Mirror Man passively and does NOT wake again at #-8-A:
    use crate::roles::vampire::copycat::COPYCAT;
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &HENCHMAN),
        ],
        [&MIRROR_MAN, &HENCHMAN, &INNOCENT_BYSTANDER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "1").await;
    let rep2 = h2.finish_night(n2).await;
    assert_eq!(rep2.effective_role_at(&h2.roster, "Alice"), "Mirror Man");
    assert!(rep2.dms_to("Alice").contains("no further actions tonight"));
}

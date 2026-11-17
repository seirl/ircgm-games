use crate::roles::base::villager::VILLAGER;
use crate::roles::super_villains::{henchman::HENCHMAN, temptress::TEMPTRESS};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn temptress_excludes_waking_villains_plants_extra_villain_and_notifies_syndicate() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &HENCHMAN),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Alice").contains("Non-Villains: Carol, Dave"));
    assert!(report
        .dms_to("Bob")
        .contains("planted the extra Villain card onto Carol"));
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Henchman #7");
}

use crate::roles::base::villager::VILLAGER;
use crate::roles::super_villains::{evilometer::EVILOMETER, henchman::HENCHMAN};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn evilometer_beeps_for_adjacent_villain_and_villains_spot_evilometer() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &HENCHMAN),
            ("Bob", &EVILOMETER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Bob").contains("*BEEP! BEEP!*"));
    assert!(report.dms_to("Alice").contains("Bob holds the Evilometer"));
}

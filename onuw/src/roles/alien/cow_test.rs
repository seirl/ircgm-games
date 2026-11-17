use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    cow::COW,
};
use crate::roles::base::villager::VILLAGER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn cow_feels_hoof_tap_when_adjacent_to_waking_alien_and_aliens_spot_cow() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &COW),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Bob").contains("*TAP!*"));
    assert!(report.dms_to("Alice").contains("Bob holds the Cow card"));
}

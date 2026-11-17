use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::daybreak::mystic_wolf::MYSTIC_WOLF;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn lone_mystic_wolf_gets_lone_wolf_center_peek_then_mystic_player_peek() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MYSTIC_WOLF),
            ("Bob", &SEER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &SEER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "2").await;
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #2 is: Seer"));
    assert!(report.dms_to("Alice").contains("Bob's card is: Seer"));
}

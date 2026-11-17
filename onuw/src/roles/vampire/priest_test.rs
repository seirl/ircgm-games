use crate::marks::Mark;
use crate::roles::base::villager::VILLAGER;
use crate::roles::vampire::{priest::PRIEST, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn priest_cleanses_self_and_another_player_with_mark_of_clarity() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &PRIEST),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Bob", "Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Clarity);
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Clarity);
}

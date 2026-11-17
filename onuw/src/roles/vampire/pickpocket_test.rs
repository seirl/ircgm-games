use crate::marks::Mark;
use crate::roles::base::villager::VILLAGER;
use crate::roles::vampire::{assassin::ASSASSIN, pickpocket::PICKPOCKET};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn pickpocket_steals_and_views_another_players_mark() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ASSASSIN),
            ("Bob", &PICKPOCKET),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Bob", "Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Assassin);
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Clarity);
    assert!(report
        .dms_to("Bob")
        .contains("your new Mark is Mark of the Assassin!"));
}

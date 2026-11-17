use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::super_villains::{dr_peeker::DR_PEEKER, henchman::HENCHMAN};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn dr_peeker_wakes_with_syndicate_and_x_rays_another_players_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DR_PEEKER),
            ("Bob", &HENCHMAN),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Fellow Super Villains: Bob"));
    assert!(report.dms_to("Alice").contains("Carol holds Seer"));
}

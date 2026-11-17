use crate::roles::base::{robber::ROBBER, seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::apprentice_seer::APPRENTICE_SEER;
use crate::roles::vampire::beholder::BEHOLDER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn beholder_sees_seer_and_apprentice_seer_thumbs_and_detects_card_swaps() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SEER),
            ("Bob", &APPRENTICE_SEER),
            ("Carol", &ROBBER),
            ("Dave", &BEHOLDER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "pass").await;
    h.reply_dm("Bob", "1").await;
    // Robber Carol steals Seer Alice's card:
    h.reply_dm("Carol", "Alice").await;
    // Beholder Dave wakes at #9Z, sees Alice & Bob's thumbs, and checks 'all':
    h.reply_dm("Dave", "all").await;

    let report = h.finish_night(night).await;
    let dave_dms = report.dms_to("Dave");
    assert!(dave_dms.contains("Alice: Robber"), "got:\n{dave_dms}");
    assert!(dave_dms.contains("Bob: Apprentice Seer"), "got:\n{dave_dms}");
}

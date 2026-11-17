use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    exposer::{ForcedExposerCount, EXPOSER},
};
use crate::roles::base::{tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::curator::CURATOR;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn exposer_all_or_nothing_flips_center_cards_visible_to_later_wakers() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &EXPOSER),
            ("Bob", &CURATOR),
            ("Carol", &ALIEN),
            ("Dave", &VILLAGER),
        ],
        [&WEREWOLF, &TANNER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    h.set_key(ForcedExposerCount(Some(2)));

    let night = h.spawn_night();
    h.reply_dm("Carol", "3").await;
    h.wait_for_prompt_dm("Alice").await;
    h.send_dm("Alice", "1").await;
    let rej = h.wait_for_new_dm("Alice").await;
    assert!(rej.contains("EXACTLY 2"), "got: {rej}");
    h.send_dm("Alice", "1 2").await;
    h.reply_dm("Bob", "pass").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    assert!(
        bob_dms.contains("Center #1 [🔦 Werewolf]") && bob_dms.contains("Center #2 [🔦 Tanner]"),
        "expected Curator to see exposed center cards, got:\n{bob_dms}"
    );
    assert!(report.board.is_center_face_up(0));
    assert!(report.board.is_center_face_up(1));
    assert!(!report.board.is_center_face_up(2));
}

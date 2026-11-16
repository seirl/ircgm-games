use std::time::Duration;

use crate::roles::base::{drunk::DRUNK, seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn drunk_swaps_with_chosen_center_card_and_narrates_it() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DRUNK),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &SEER, &WEREWOLF],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "c3").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert_eq!(report.center_role(3), "Drunk");
    assert!(report.has_chronicle_substr("Drunk (Alice) swapped Drunk with Center #3 (Werewolf)."));
}

#[tokio::test(start_paused = true)]
async fn drunk_timeout_auto_swaps_with_center() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DRUNK),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &WEREWOLF],
    )
    .await;

    let night = h.spawn_night();
    let _ = h.wait_for_new_dm("Alice").await;
    h.advance_time(Duration::from_secs(10)).await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert!(report.has_chronicle_substr("Drunk (Alice) swapped Drunk with Center #"));
}

#[tokio::test(start_paused = true)]
async fn shielded_drunk_skips_center_prompt_without_contradictory_dms() {
    use crate::roles::daybreak::sentinel::SENTINEL;

    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SENTINEL),
            ("Bob", &DRUNK),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &WEREWOLF],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Drunk");
    let bob_dms = report.dms_to("Bob");
    assert!(bob_dms.contains("Your own card has a Shield token"));
    assert!(!bob_dms.contains("Randomly swapped"));
    assert!(!bob_dms.contains("You MUST swap"));
    assert!(report
        .has_chronicle_substr("Drunk (Bob) could not swap because their own card was shielded."));
}

use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::daybreak::{alpha_wolf::ALPHA_WOLF, mystic_wolf::MYSTIC_WOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn alpha_wolf_and_mystic_wolf_wake_together_even_without_base_werewolf_in_deck() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &MYSTIC_WOLF),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Alpha Wolf) bites Carol; Bob (Mystic Wolf) inspects Carol; Carol (Seer) passes:
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Bob", "Carol").await;

    let report = h.finish_night(night).await;
    // Pack recognition happened automatically via companion_group_plugin(&WEREWOLF):
    assert!(report.dms_to("Alice").contains("Fellow Werewolf pack: Bob"));
    assert!(report.dms_to("Bob").contains("Fellow Werewolf pack: Alice"));
    // Alpha Wolf (122) bit Carol BEFORE Mystic Wolf (123) resolved, so Carol became Werewolf!
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert!(report.dms_to("Bob").contains("Carol's card is: Werewolf"));
    assert!(report.has_chronicle_substr("Alpha Wolf (Alice) bit Carol, swapping Werewolf in for Seer."));
}

#[tokio::test(start_paused = true)]
async fn lone_mystic_wolf_gets_lone_wolf_center_peek_then_mystic_peek_cleanly() {
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
    // Tier 1 (Werewolf pack wake, 120): Lone Wolf Alice center peek:
    h.reply_dm("Alice", "2").await;
    // Tier 2 (Mystic Wolf 123 + Seer 150): Alice peeks Bob, Bob passes:
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #2 is: Seer"));
    assert!(report.dms_to("Alice").contains("Bob's card is: Seer"));
}

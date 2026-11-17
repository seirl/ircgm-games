use std::time::Duration;

use crate::roles::base::{seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn lone_wolf_rejects_bad_slot_then_acknowledges_choice_immediately_and_peeks_at_tier_end() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &SEER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "99").await;
    let reject = h.wait_for_new_dm("Alice").await;
    assert!(reject.contains("Reply 1, 2, 3, or pass."));

    h.advance_time(Duration::from_secs(1)).await;
    h.send_dm("Alice", "2").await;
    let ack = h.wait_for_new_dm("Alice").await;
    assert!(ack.contains("Locked in Center #2"));
    let ack_ts = h.timed_dms["Alice"]
        .iter()
        .find(|(_, m)| m.contains("Locked in Center #2"))
        .unwrap()
        .0;
    assert_eq!(ack_ts, Duration::from_secs(1));

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #2 is: Seer"));
    assert!(report.has_chronicle_substr("Lone Wolf Alice peeked Center #2 (Seer)."));
}

#[tokio::test(start_paused = true)]
async fn two_werewolves_see_each_other_without_center_peek() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &SEER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    let _ = h.wait_for_new_dm("Alice").await;
    let _ = h.wait_for_new_dm("Bob").await;
    // Talking at night is forbidden in ONUW — Werewolves recognize each other silently without chat relay!
    h.send_dm("Alice", "I'll claim Seer!").await;
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Fellow Werewolf pack: Bob"));
    assert!(report.dms_to("Bob").contains("Fellow Werewolf pack: Alice"));
    assert!(!report.dms_to("Bob").contains("I'll claim Seer!"));
    assert!(report.has_chronicle_substr("Werewolves (Alice, Bob) recognized each other."));
}

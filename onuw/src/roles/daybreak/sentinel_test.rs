use crate::roles::base::{robber::ROBBER, seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::{sentinel::SENTINEL, village_idiot::VILLAGE_IDIOT};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn sentinel_shield_blocks_seer_robber_and_village_idiot_from_shielded_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SENTINEL),
            ("Bob", &WEREWOLF),
            ("Carol", &SEER),
            ("Dave", &ROBBER),
            ("Eve", &VILLAGE_IDIOT),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Stage A (Dusk): Alice shields Bob!
    h.reply_dm("Alice", "Bob").await;

    // Stage B (Night):
    // Bob (Lone Wolf) passes center peek:
    h.reply_dm("Bob", "pass").await;
    // Carol (Seer) tries to peek shielded Bob -> rejected! Then peeks Dave:
    h.reply_dm("Carol", "Bob").await;
    h.send_dm("Carol", "Dave").await;
    // Dave (Robber) tries to rob shielded Bob -> rejected! Then passes:
    h.reply_dm("Dave", "Bob").await;
    h.send_dm("Dave", "pass").await;
    // Eve (Village Idiot) rotates cards right — shielded Bob and Eve herself stay put,
    // while Alice (0), Carol (2), Dave (3) rotate right!
    h.reply_dm("Eve", "right").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Carol").contains("Shield token"));
    assert!(report.dms_to("Dave").contains("shielded by the Sentinel"));
    // Bob was shielded so Bob STILL holds Werewolf!
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Werewolf");
    // Unshielded ring [Alice=Sentinel, Carol=Seer, Dave=Robber] rotated right ->
    // Alice gets Dave's Robber, Carol gets Alice's Sentinel, Dave gets Carol's Seer!
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Robber");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Sentinel");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Seer");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Village Idiot");
    assert!(report.has_chronicle_substr("Sentinel (Alice) placed a Shield token on Bob's card."));
    assert!(report.has_chronicle_substr("Village Idiot (Eve) rotated 3 unshielded cards right."));
}

#[tokio::test(start_paused = true)]
async fn dusk_doppelganger_copying_sentinel_wakes_once_sequentially_after_sentinel() {
    use crate::roles::base::doppelganger::DOPPELGANGER;
    use crate::roles::vampire::vampire::VAMPIRE;

    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &SENTINEL),
            ("Carol", &VAMPIRE),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Dusk #-7: Alice copies Sentinel Bob; Dusk #-6: Vampire Carol marks Dave:
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Carol", "Dave").await;
    // Night #0: Sentinel Bob shields Carol; Night #1: Doppelgänger-Sentinel Alice wakes ONCE and shields Dave:
    h.reply_dm("Bob", "Carol").await;
    h.reply_dm("Alice", "Dave").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.board.shielded_seats().len(), 2);
    assert!(report.board.is_shielded(h.seat_of("Carol")));
    assert!(report.board.is_shielded(h.seat_of("Dave")));
}

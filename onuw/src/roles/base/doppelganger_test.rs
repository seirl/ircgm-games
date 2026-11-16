use crate::roles::base::{
    doppelganger::DOPPELGANGER, insomniac::INSOMNIAC, minion::MINION, robber::ROBBER, seer::SEER,
    villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::roles::daybreak::mystic_wolf::MYSTIC_WOLF;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn doppelganger_copies_robber_acts_immediately_and_chronicle_records_both_steps() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &ROBBER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Doppelgänger) copies Bob (Robber), then immediately steals Carol's Werewolf card:
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;
    // Meanwhile Bob (the original Robber) passes:
    h.reply_dm("Bob", "pass").await;
    // Carol (Lone Wolf) passes center peek:
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Robber");
    assert!(report.has_chronicle_substr("Doppelgänger (Alice) copied Bob and became Robber."));
    assert!(report.has_chronicle_substr(
        "Doppelgänger-Robber (Alice) stole Werewolf from Carol (giving Carol Doppelgänger)."
    ));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_insomniac_wakes_at_tail_of_night_and_sees_swapped_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &INSOMNIAC),
            ("Carol", &ROBBER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Carol (Robber) steals Alice's card while Alice copies Bob (Insomniac):
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Carol", "Alice").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Robber");
    // Carol stole the physical Doppelgänger card (which had copied Insomniac), so Carol's effective role is Insomniac,
    // while Carol saw the physical "Doppelgänger" face when robbing!
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Insomniac");
    assert!(report.dms_to("Carol").contains("got: Doppelgänger!"));
    // Alice (Doppelgänger-Insomniac) wakes after all mutations and sees she now holds Robber:
    assert!(report
        .dms_to("Alice")
        .contains("Doppelgänger-Insomniac: You check your card before dawn — it is Robber!"));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_werewolf_joins_pack_wake_and_is_seen_by_minion() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &WEREWOLF),
            ("Carol", &MINION),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Fellow Werewolf pack: Bob"));
    assert!(report.dms_to("Bob").contains("Fellow Werewolf pack: Alice"));
    assert!(report
        .dms_to("Carol")
        .contains("The Werewolves are Alice, Bob"));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_mystic_wolf_wakes_with_pack_first_then_peeks_on_mystic_wolf_turn() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MYSTIC_WOLF),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Doppelgänger) copies Bob (Mystic Wolf):
    // Because Mystic Wolf uses `CopyTiming::OnRoleTurn` (#2C), Alice does NOT peek blind in the
    // immediate window — instead Alice joins Bob in the Werewolf pack huddle (#2) first, and BOTH
    // Mystic Wolves peek in the post-pack tier (#2C)!
    h.reply_dm("Alice", "Bob").await;
    // In the Mystic Wolf tier (#2C), Bob peeks Carol and Alice (Doppelgänger-Mystic Wolf) also peeks Carol;
    // Carol (Seer, #5A) runs concurrently in the same tier and peeks Bob:
    h.reply_dm("Bob", "Carol").await;
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Carol", "Bob").await;

    let report = h.finish_night(night).await;
    let alice_plain = report.dms_to("Alice");
    let pack_pos = alice_plain
        .find("Fellow Werewolf pack: Bob")
        .expect("Alice woke in Werewolf pack huddle");
    let peek_pos = alice_plain
        .find("Mystic Wolf: Reply")
        .expect("Alice got Mystic Wolf solo peek prompt");
    assert!(
        pack_pos < peek_pos,
        "Doppelgänger-Mystic Wolf must see packmates before being prompted for Mystic Wolf peek"
    );
    assert!(alice_plain.contains("Carol's card is: Seer"));
    assert!(report.has_chronicle_substr("Doppelgänger-Mystic Wolf (Alice) viewed Carol's card (Seer)."));
}

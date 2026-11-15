use crate::roles::base::{
    doppelganger::DOPPELGANGER, insomniac::INSOMNIAC, minion::MINION, robber::ROBBER,
    villager::VILLAGER, werewolf::WEREWOLF,
};
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

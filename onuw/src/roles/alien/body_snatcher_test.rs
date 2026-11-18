use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    body_snatcher::{BodySnatcherMode, ForcedBodySnatcherMode, BODY_SNATCHER},
    oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE},
};
use crate::roles::base::{doppelganger::DOPPELGANGER, villager::VILLAGER};
use crate::roles::vampire::count::COUNT;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn body_snatcher_assimilates_stolen_card_into_alien_so_both_players_become_aliens() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &BODY_SNATCHER),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    h.set_key(ForcedBodySnatcherMode(Some(BodySnatcherMode::StealPlayer)));

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Alice").contains("is now an Alien too"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);

    // Body Snatcher + Doppelgänger-Body Snatcher run sequentially:
    // Slot 2 (#1-F-A) STILL rejects known waking fellow Alien Alice (who woke together at #1-A),
    // but does NOT leak or reject Bob (whom Alice secretly assimilated in Slot 1 while Carol's eyes were closed)!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &BODY_SNATCHER),
            ("Bob", &VILLAGER),
            ("Carol", &DOPPELGANGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    h2.set_key(ForcedBodySnatcherMode(Some(BodySnatcherMode::StealPlayer)));

    let n2 = h2.spawn_night();
    h2.reply_dm("Carol", "Alice").await; // Doppelgänger -> Body Snatcher (2 Aliens wake together at #1-A)
    // Slot 1 (#1-F): Alice snatches Bob (Villager), secretly giving Bob her Body Snatcher card
    h2.reply_dm("Alice", "Bob").await;
    // Slot 2 (#1-F-A): Carol wakes; Eligible must include BOTH Bob and Dave (no leak of Alice's target!),
    // while rejecting Alice (known waking Alien from #1-A)!
    let carol_prompt = h2.wait_for_prompt_dm("Carol").await;
    assert!(
        carol_prompt.contains("Bob") && carol_prompt.contains("Dave") && !carol_prompt.contains("Eligible: Alice"),
        "expected Bob & Dave eligible without leaking Slot 1 target, got: {carol_prompt}"
    );
    h2.send_dm("Carol", "Alice").await;
    let rej = h2.wait_for_new_dm("Carol").await;
    assert!(rej.contains("fellow Alien"), "got: {rej}");
    // Carol snatches Bob (Slot 1's victim, now holding Alice's Body Snatcher card):
    // Bob receives Carol's Doppelgänger [Body Snatcher] card, and Carol assimilates Bob's Body Snatcher card!
    h2.send_dm("Carol", "Bob").await;
    let rep2 = h2.finish_night(n2).await;

    assert!(rep2.dms_to("Carol").contains("snatched Bob's Body Snatcher"));
    assert_eq!(rep2.effective_role_at(&h2.roster, "Alice"), "Alien");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Bob"), "Body Snatcher");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Carol"), "Alien");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Dave"), "Villager");
}

#[tokio::test(start_paused = true)]
async fn body_snatcher_does_not_leak_feared_aliens_or_blind_oracle_alien_swaps() {
    // 1. Count paralyzes Alien Bob with Mark::Fear in Dusk (#-6B), so Bob never wakes at #1-A:
    //    Body Snatcher Alice must NOT have Bob filtered out or rejected as a fellow Alien!
    let mut h_fear = OnuwTestHarness::new(
        &[
            ("Alice", &BODY_SNATCHER),
            ("Bob", &ALIEN),
            ("Carol", &COUNT),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_fear.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    h_fear.set_key(ForcedBodySnatcherMode(Some(BodySnatcherMode::StealPlayer)));

    let n_fear = h_fear.spawn_night();
    // Dusk #-6 Vampires (Mark of Vampire on Dave) + #-6B Count (Mark of Fear on Bob):
    h_fear.reply_dm("Carol", "Dave").await;
    h_fear.reply_dm("Carol", "Bob").await;
    let alice_prompt = h_fear.wait_for_prompt_dm("Alice").await;
    assert!(
        alice_prompt.contains("Bob"),
        "feared Alien Bob must remain eligible (not leaked), got: {alice_prompt}"
    );
    h_fear.send_dm("Alice", "Bob").await;
    let rep_fear = h_fear.finish_night(n_fear).await;
    assert_eq!(rep_fear.effective_role_at(&h_fear.roster, "Alice"), "Alien");
    assert_eq!(
        rep_fear.effective_role_at(&h_fear.roster, "Bob"),
        "Body Snatcher"
    );

    // 2. Oracle Alice blindly swaps with Center #1 (Alien) via DrunkExchange at #-9 without waking at #1-A:
    //    Body Snatcher Bob must NOT have Alice excluded from Eligible!
    let mut h_oracle = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &BODY_SNATCHER),
            ("Carol", &VILLAGER),
        ],
        [&ALIEN, &ALIEN, &ALIEN],
    )
    .await;
    h_oracle.set_key(ForcedOracleQuestion(Some(OracleQuestion::DrunkExchange {
        fizzle: false,
    })));
    h_oracle.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    h_oracle.set_key(ForcedBodySnatcherMode(Some(BodySnatcherMode::StealPlayer)));

    let n_oracle = h_oracle.spawn_night();
    h_oracle.reply_dm("Alice", "1").await;
    let bob_prompt = h_oracle.wait_for_prompt_dm("Bob").await;
    assert!(
        bob_prompt.contains("Alice"),
        "blind-swapped Oracle holding Alien card must not be leaked, got: {bob_prompt}"
    );
    h_oracle.send_dm("Bob", "Alice").await;
    let rep_oracle = h_oracle.finish_night(n_oracle).await;
    assert_eq!(rep_oracle.effective_role_at(&h_oracle.roster, "Bob"), "Alien");
    assert_eq!(
        rep_oracle.effective_role_at(&h_oracle.roster, "Alice"),
        "Body Snatcher"
    );
}

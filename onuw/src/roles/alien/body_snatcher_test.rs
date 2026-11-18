use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent},
    body_snatcher::{BodySnatcherMode, ForcedBodySnatcherMode, BODY_SNATCHER},
};
use crate::roles::base::villager::VILLAGER;
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
    h.reply_dm("Alice", "1").await;
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

    // Body Snatcher + Doppelgänger-Body Snatcher run sequentially and cannot snatch an Alien card
    // off a player already assimilated by the first Body Snatcher:
    use crate::roles::base::doppelganger::DOPPELGANGER;
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
    h2.reply_dm("Carol", "Alice").await; // Doppelgänger -> Body Snatcher (2 Aliens at #1-A, so no Lone Alien center prompt!)
    // Slot 1 (#1-F): Alice snatches Bob (Villager), giving Bob her Body Snatcher card
    h2.reply_dm("Alice", "Bob").await;
    // Slot 2 (#1-F-A): Carol wakes; Bob now holds an Alien card so snatching Bob is rejected!
    h2.wait_for_prompt_dm("Carol").await;
    h2.send_dm("Carol", "Bob").await;
    let rej = h2.wait_for_new_dm("Carol").await;
    assert!(rej.contains("fellow Alien"), "got: {rej}");
    h2.send_dm("Carol", "Dave").await;
    let rep2 = h2.finish_night(n2).await;

    assert_eq!(rep2.effective_role_at(&h2.roster, "Alice"), "Alien");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Bob"), "Body Snatcher");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Carol"), "Alien");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Dave"), "Body Snatcher");
}

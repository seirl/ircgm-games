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
}

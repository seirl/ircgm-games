use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    groob_zerb::{GROOB, ZERB},
    leader::LEADER,
};
use crate::roles::base::villager::VILLAGER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn leader_unanimous_alien_point_steals_victory_and_groob_zerb_survival_win_works() {
    // 1. Unanimous Alien vote on Leader steals Alien team victory even when an Alien dies:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &ALIEN),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let night = h.spawn_night();
    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Carol")
        .contains("The Aliens are Alice, Bob"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);

    // 2. Both Groob & Zerb in play -> Leader wins iff neither Groob nor Zerb dies:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &GROOB),
            ("Bob", &ZERB),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedAlienEvent(Some(AlienEvent::CenterProbe)));
    let night2 = h2.spawn_night();
    h2.reply_dm("Alice", "!pass").await;
    let report2 = h2.finish_night(night2).await;
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol"]);
}

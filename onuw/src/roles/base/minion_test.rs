use crate::roles::base::{minion::MINION, tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::dream_wolf::DREAM_WOLF;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn minion_sees_both_waking_and_dream_wolves() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MINION),
            ("Bob", &WEREWOLF),
            ("Carol", &DREAM_WOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Bob is the sole *waking* wolf (Carol sleeps as Dream Wolf), so Bob gets Lone Wolf prompt:
    h.reply_dm("Bob", "pass").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("The Werewolves are Bob, Carol"));
}

#[tokio::test(start_paused = true)]
async fn solo_minion_with_zero_wolves_follows_official_onuw_victory_rules() {
    // Case 1: Nobody dies (1-1-1 circle vote) -> Village wins, Minion loses!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MINION),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    let report = h.finish_night(night).await;
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);

    // Case 2: Villager Bob dies while Minion Alice survives -> Minion wins!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &MINION),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    let report2 = h2.finish_night(night2).await;
    let dawn2 = h2
        .resolve_dawn(
            report2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Alice"]);

    // Case 3: Tanner dies while Minion survives -> ONLY Tanner wins (blocks Evil)!
    let mut h3 = OnuwTestHarness::new(
        &[("Alice", &MINION), ("Bob", &TANNER), ("Carol", &VILLAGER)],
        [&WEREWOLF, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night3 = h3.spawn_night();
    let report3 = h3.finish_night(night3).await;
    let dawn3 = h3
        .resolve_dawn(
            report3,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Bob"]);
}

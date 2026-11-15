use crate::roles::base::{tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn tanner_blocks_werewolves_when_killed_alone_and_shares_win_with_village_when_wolf_also_dies() {
    // 1. Tanner killed alone -> ONLY Tanner wins!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &TANNER),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice"]);

    // 2. Tie 2-2 between Tanner (Alice) and Werewolf (Bob) -> BOTH Tanner and Village win!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &TANNER),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Bob", "pass").await;
    let report2 = h2.finish_night(night2).await;
    let dawn2 = h2
        .resolve_dawn(
            report2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Alice", "Carol", "Dave"]);
}

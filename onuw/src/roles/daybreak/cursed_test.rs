use crate::roles::base::{minion::MINION, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::cursed::CURSED;
use crate::roles::vampire::vampire::VAMPIRE;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn cursed_converts_when_werewolf_votes_for_them_and_gives_village_win_if_killed() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURSED),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "1").await;
    let report = h.finish_night(night).await;

    // Bob (Werewolf) & Carol vote Alice (Cursed) -> Alice turns into a Werewolf and dies, so Village wins!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;

    let dead = dawn.dead_nicks(&h.roster);
    assert!(dead.contains("Alice"));
    assert!(dead.contains("Bob"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Carol", "Dave"]);
}

#[tokio::test(start_paused = true)]
async fn cursed_surviving_after_werewolf_vote_wins_with_werewolves_and_minion_vote_does_not_convert()
{
    // 1. Werewolf votes Cursed (1 vote, not enough to kill) while Villager dies -> Cursed wins with Werewolf!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURSED),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Bob", "1").await;
    let report = h.finish_night(night).await;
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);

    // 2. Minion voting Cursed (without Werewolf voting Cursed) does NOT convert Cursed!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &CURSED),
            ("Bob", &WEREWOLF),
            ("Carol", &MINION),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Bob", "1").await;
    let report2 = h2.finish_night(night2).await;
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    // Alice (2 votes from Minion + Villager) & Dave (2 votes) both die; Bob (Werewolf) survives -> Alice stayed Village and lost!
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Bob", "Carol"]);
}

#[tokio::test(start_paused = true)]
async fn cursed_converts_to_vampire_when_voted_by_vampire() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURSED),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    // Vampire Bob marks Carol with Mark of the Vampire in Dusk:
    h.reply_dm("Bob", "Carol").await;
    let report = h.finish_night(night).await;

    // Bob (Vampire) points at Alice (Cursed) while Dave dies -> Alice converts to Vampire and wins with Bob & Carol!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Carol"]);
}

#[tokio::test(start_paused = true)]
async fn cursed_converts_to_faction_casting_more_monster_votes_when_wolves_and_vampires_both_point() {
    // 2 Vampires (Bob + bitten Carol) AND 1 Werewolf (Dave) point at Alice (Cursed):
    // 2 Vampire votes > 1 Werewolf vote -> Alice turns into a Vampire (not a Werewolf) and dies,
    // while Dave (Werewolf) also dies (3 votes) -> Village Eve wins because both a Vampire (Alice) and Werewolf (Dave) died!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURSED),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
            ("Dave", &WEREWOLF),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Bob", "Carol").await;
    h.reply_dm("Dave", "1").await;
    let report = h.finish_night(night).await;

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Dave")),
            ],
        )
        .await;
    assert_eq!(
        dawn.summary.final_board.player_card(0).effective_role().name(),
        "Vampire"
    );
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Eve"]);
}

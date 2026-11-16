use crate::roles::base::{
    drunk::DRUNK, hunter::HUNTER, insomniac::INSOMNIAC, mason::MASON, robber::ROBBER, seer::SEER,
    tanner::TANNER, troublemaker::TROUBLEMAKER, villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::testing::OnuwTestHarness;
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn seer_robber_troublemaker_drunk_and_insomniac_resolve_in_canonical_wake_order() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SEER),
            ("Bob", &ROBBER),
            ("Carol", &TROUBLEMAKER),
            ("Dave", &DRUNK),
            ("Eve", &INSOMNIAC),
        ],
        [&WEREWOLF, &MASON, &MASON],
    )
    .await;

    let night = h.spawn_night();
    // Even if Troublemaker answers BEFORE Robber in wall-clock order, causal DAG commits
    // Seer (150) -> Robber (160) -> Troublemaker (170) -> Drunk (180) -> Insomniac (190)!
    h.reply_dm("Carol", "Bob Eve").await;
    h.reply_dm("Alice", "center 1 2").await;
    h.reply_dm("Bob", "Alice").await;
    h.reply_dm("Dave", "1").await;

    let report = h.finish_night(night).await;
    // Trace canonical card movements:
    // Initial: Alice=Seer, Bob=Robber, Carol=Troublemaker, Dave=Drunk, Eve=Insomniac | C1=Werewolf, C2=Mason
    // 1. Seer (Alice) sees C1=Werewolf & C2=Mason.
    // 2. Robber (Bob) steals Alice's Seer -> Alice=Robber, Bob=Seer (Bob told he got Seer!).
    // 3. Troublemaker (Carol) swaps Bob (now Seer!) and Eve (Insomniac) -> Bob=Insomniac, Eve=Seer.
    // 4. Drunk (Dave) swaps with C1 (Werewolf) -> Dave=Werewolf, C1=Drunk.
    // 5. Insomniac (Eve) wakes at 190 and sees she now holds Seer!
    assert!(report
        .dms_to("Alice")
        .contains("Center #1 is Werewolf, Center #2 is Mason."));
    assert!(report.dms_to("Bob").contains("robbed Alice and got: Seer!"));
    assert!(report
        .dms_to("Eve")
        .contains("Insomniac: You check your card before dawn — it is Seer!"));

    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Robber");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Insomniac");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Troublemaker");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Seer");
    assert_eq!(report.center_role(1), "Drunk");

    // Verify full causal chronicle order!
    let chron = report.full_chronicle_text();
    assert!(chron.contains("Seer (Alice) viewed Center #1 (Werewolf) & Center #2 (Mason)."));
    assert!(chron.contains("Robber (Bob) stole Seer from Alice (giving Alice Robber)."));
    assert!(chron.contains("Troublemaker (Carol) swapped Bob (Seer) and Eve (Insomniac)."));
    assert!(chron.contains("Drunk (Dave) swapped Drunk with Center #1 (Werewolf)."));
    assert!(chron.contains("Insomniac (Eve) checked their card and saw Seer."));
}

#[tokio::test(start_paused = true)]
async fn drunk_timeout_auto_swaps_with_center() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DRUNK),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &WEREWOLF],
    )
    .await;

    let night = h.spawn_night();
    let _ = h.wait_for_new_dm("Alice").await;
    h.advance_time(Duration::from_secs(10)).await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert!(report.has_chronicle_substr("Drunk (Alice) swapped Drunk with Center #"));
}

#[tokio::test(start_paused = true)]
async fn hunter_death_shot_kills_voted_werewolf_and_tanner_co_victory_works() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &HUNTER),
            ("Bob", &WEREWOLF),
            ("Carol", &TANNER),
            ("Dave", &MASON),
        ],
        [&MASON, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Dave").contains("ONLY Mason"));

    // Tie 2-2 between Hunter (Alice) and Tanner (Carol), while Hunter points at Werewolf (Bob)!
    // -> Alice and Carol die from the vote, and Alice's Hunter shot kills Bob (Werewolf)!
    // -> Both Tanner (Carol) AND Village (Alice, Dave) win!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    // Wait: tally is Alice=2, Bob=1, Carol=1 -> only Alice gets 2 votes, Alice shoots Bob!
    assert!(dawn.dead_nicks(&h.roster).contains("Alice"));
    assert!(dawn.dead_nicks(&h.roster).contains("Bob"));
    assert!(dawn.channel_text().contains("HUNTER SHOT!"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Dave"]);
}

use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::{
    aura_seer::AURA_SEER, bodyguard::BODYGUARD, curator::CURATOR,
    paranormal_investigator::PARANORMAL_INVESTIGATOR, revealer::REVEALER, witch::WITCH,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn paranormal_investigator_peeks_villager_then_transforms_on_werewolf() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &PARANORMAL_INVESTIGATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    // Peek 1/2: Bob is Villager -> continues to Peek 2/2: Carol is Werewolf -> transforms!
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    // Face role on the physical card is still Paranormal Investigator:
    assert_eq!(
        report.face_role_at(&h.roster, "Alice"),
        "Paranormal Investigator"
    );
    assert!(report.has_chronicle_substr(
        "Paranormal Investigator (Alice) inspected Bob, Carol and transformed into Werewolf!"
    ));
}

#[tokio::test(start_paused = true)]
async fn witch_revealer_curator_aura_seer_and_bodyguard_work_end_to_end() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WITCH),
            ("Bob", &REVEALER),
            ("Carol", &CURATOR),
            ("Dave", &AURA_SEER),
            ("Eve", &BODYGUARD),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Witch (Alice, 165) peeks Center #1 (Werewolf) and swaps it onto Carol!
    h.reply_dm("Alice", "1").await;
    h.reply_dm("Alice", "Carol").await;
    // Curator (Carol, 185) passes:
    h.reply_dm("Carol", "pass").await;
    // Revealer (Bob, 192) flips Eve (Bodyguard -> Village, stays face-up!):
    h.reply_dm("Bob", "Eve").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert!(report.board.face_up_players.contains(&h.seat_of("Eve")));
    // Aura Seer (Dave) senses Alice (Witch) and Bob (Revealer)!
    assert!(report
        .dms_to("Dave")
        .contains("Players who viewed or moved a card tonight: Alice, Bob"));

    // At Dawn: Carol (now Werewolf!) gets 2 votes, Alice gets 3 votes,
    // BUT Eve (Bodyguard) points at Alice! So Alice survives and Carol (runner-up with 2 votes) dies!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    // Wait: Carol only got 1 vote above — let's check! If Alice got 4 votes and Carol got 1 vote, nobody dies because runner-up < 2!
    assert!(dawn.dead_nicks(&h.roster).is_empty());
    assert!(dawn.channel_text().contains("shielded Alice from execution"));
}

#[tokio::test(start_paused = true)]
async fn bodyguard_deflects_execution_to_runner_up_with_at_least_two_votes() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &BODYGUARD),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    let report = h.finish_night(night).await;

    // Bob gets 3 votes (Carol, Dave, Eve), Carol (Werewolf) gets 2 votes (Alice? No, Alice=Bodyguard votes Bob to protect Bob!).
    // Wait: let's have 6 players or have Bob=2, Carol=2:
    // Bob gets 2 votes (Alice=Bodyguard, Carol), Carol gets 2 votes (Bob, Dave), Alice gets 1 vote (Eve):
    // Bodyguard Alice pointed at Bob -> Bob is protected, so ONLY Carol (2 votes) dies and Village wins!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn.dead_nicks(&h.roster),
        ["Carol".to_owned()].into_iter().collect()
    );
    assert_eq!(
        dawn.winner_nicks(&h.roster),
        vec!["Alice", "Bob", "Dave", "Eve"]
    );
}

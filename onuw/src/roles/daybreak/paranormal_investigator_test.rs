use crate::roles::base::{minion::MINION, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::{
    aura_seer::AURA_SEER, bodyguard::BODYGUARD, curator::CURATOR, dream_wolf::DREAM_WOLF,
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
    // Revealer (Bob, #10 / 192) flips Eve (Bodyguard -> Village, stays face-up!):
    h.reply_dm("Bob", "Eve").await;
    // Curator (Carol, #11 / 194) passes:
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert!(report.board.is_player_face_up(h.seat_of("Eve")));
    // Aura Seer (Dave, #7C) wakes after Witch (Alice, #6B) and BEFORE Revealer (Bob, #10),
    // so Aura Seer senses Alice (Witch) and does NOT detect Revealer!
    assert!(report
        .dms_to("Dave")
        .contains("Players who viewed or moved a card tonight: Alice"));
    assert!(!report.dms_to("Dave").contains("Alice, Bob"));

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

#[tokio::test(start_paused = true)]
async fn dream_wolf_suppresses_lone_wolf_and_minion_does_not_trigger_pi_or_revealer_flip_back() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &DREAM_WOLF),
            ("Carol", &MINION),
            ("Dave", &PARANORMAL_INVESTIGATOR),
            ("Eve", &REVEALER),
            ("Frank", &CURATOR),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Werewolf) sees Dream Wolf Bob and gets NO Lone Wolf center peek!
    // Dave (PI) peeks Carol (Minion -> does NOT transform!), then peeks Alice (Werewolf -> transforms!):
    h.reply_dm("Dave", "Carol").await;
    h.reply_dm("Dave", "Alice").await;
    // Eve (Revealer) flips Carol (Minion -> stays face-up because Minion is not Werewolf/Tanner!):
    h.reply_dm("Eve", "Carol").await;
    // Frank (Curator) tries to place an Artifact on face-up Carol (rejected!), then places on Bob:
    h.reply_dm("Frank", "Carol").await;
    let reject = h.wait_for_new_dm("Frank").await;
    assert!(reject.contains("Face-up"));
    h.send_dm("Frank", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Dream Wolf Bob"));
    assert!(!report.dms_to("Alice").contains("LONE WOLF"));
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Werewolf");
    assert!(report.board.is_player_face_up(h.seat_of("Carol")));
    assert!(report.board.artifacts.contains_key(&h.seat_of("Bob")));
}

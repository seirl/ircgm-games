use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::{curator::CURATOR, revealer::REVEALER};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn curator_places_artifact_on_target_and_notifies_them() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    let art = report
        .artifact_at(&h.roster, "Bob")
        .expect("Bob received an artifact");
    assert!(report.dms_to("Bob").contains(art.name()));
    assert!(report
        .has_chronicle_substr(&format!("Curator (Alice) placed {} on Bob.", art.name())));
}

#[tokio::test(start_paused = true)]
async fn curator_can_place_artifact_on_face_up_card_flipped_by_revealer() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &REVEALER),
            ("Bob", &VILLAGER),
            ("Carol", &CURATOR),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Revealer (Alice, #10 / 192) flips Bob's Villager card face-up:
    h.reply_dm("Alice", "Bob").await;
    // Curator (Carol, #11 / 194) wakes in the next tier, sees `Bob [🔦 Villager]` on the table,
    // and is allowed per official Daybreak rules to place an Artifact on face-up Bob!
    let curator_prompt = h.wait_for_new_dm("Carol").await;
    assert!(curator_prompt.contains("Bob [🔦 Villager]"));
    h.send_dm("Carol", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report.board.is_player_face_up(h.seat_of("Bob")));
    let art = report
        .artifact_at(&h.roster, "Bob")
        .expect("Face-up Bob received an artifact");
    assert!(report.dms_to("Bob").contains(art.name()));
}

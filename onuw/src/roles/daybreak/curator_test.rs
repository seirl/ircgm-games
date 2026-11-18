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

#[tokio::test(start_paused = true)]
async fn curator_gates_drawn_artifacts_by_deck_monster_factions() {
    use crate::artifacts::Artifact;
    use crate::roles::{
        alien::alien::{AlienEvent, ForcedAlienEvent, ALIEN},
        super_villains::temptress::TEMPTRESS,
        vampire::vampire::VAMPIRE,
    };

    // 1. Super Villains + Curator (`Temptress`, 0 Werewolves/Vampires/Aliens):
    //    Curator never draws Claw, Mist, or Alien Artifact across repeated placements.
    for _ in 0..25 {
        let mut h = OnuwTestHarness::new(
            &[
                ("Alice", &CURATOR),
                ("Bob", &TEMPTRESS),
                ("Carol", &VILLAGER),
            ],
            [&VILLAGER, &VILLAGER, &VILLAGER],
        )
        .await;
        let night = h.spawn_night();
        h.reply_dm("Bob", "Carol").await;
        h.reply_dm("Alice", "Carol").await;
        let report = h.finish_night(night).await;
        let art = report.artifact_at(&h.roster, "Carol").unwrap();
        assert_ne!(art, Artifact::ClawOfTheWerewolf);
        assert_ne!(art, Artifact::MistOfTheVampire);
        assert_ne!(art, Artifact::AlienArtifact);
    }

    // 2. Vampire + Alien + Curator (0 Werewolves): Curator never draws Claw of the Werewolf.
    for _ in 0..15 {
        let mut h_va = OnuwTestHarness::new(
            &[
                ("Alice", &CURATOR),
                ("Bob", &VAMPIRE),
                ("Carol", &ALIEN),
            ],
            [&VILLAGER, &VILLAGER, &VILLAGER],
        )
        .await;
        h_va.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
        let n_va = h_va.spawn_night();
        h_va.reply_dm("Bob", "Alice").await;
        h_va.reply_dm("Alice", "Bob").await;
        let rep_va = h_va.finish_night(n_va).await;
        let placed = rep_va.artifact_at(&h_va.roster, "Bob").unwrap();
        assert_ne!(placed, Artifact::ClawOfTheWerewolf);
    }

    // 3. Curator (#11) Artifact DM is delivered at Daybreak (AFTER #13-A Mortician wakes & acts):
    use crate::roles::alien::mortician::{ForcedMorticianMode, MorticianMode, MORTICIAN};
    let mut h_mort = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &MORTICIAN),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;
    h_mort.set_key(ForcedMorticianMode(Some(MorticianMode::ChooseNeighbor)));
    let n_mort = h_mort.spawn_night();
    h_mort.reply_dm("Alice", "Bob").await;
    // When Mortician Bob receives his #13-A prompt, he must NOT yet have been told his Artifact's secret identity!
    let mort_prompt = h_mort.wait_for_prompt_dm("Bob").await;
    assert!(mort_prompt.contains("Mortician"));
    h_mort.send_dm("Bob", "pass").await;
    let rep_mort = h_mort.finish_night(n_mort).await;
    let bob_timed = &rep_mort.timed_dms["Bob"];
    let mort_prompt_idx = bob_timed
        .iter()
        .position(|(_, m)| m.contains("Mortician:"))
        .unwrap();
    let art_dm_idx = bob_timed
        .iter()
        .position(|(_, m)| m.contains("Artifact") || m.contains("You have the"))
        .unwrap();
    assert!(art_dm_idx > mort_prompt_idx);
}

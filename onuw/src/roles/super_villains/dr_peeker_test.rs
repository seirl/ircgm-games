use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::super_villains::{
    detector::DETECTOR, dr_peeker::DR_PEEKER, henchman::HENCHMAN, temptress::TEMPTRESS,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn dr_peeker_wakes_with_syndicate_and_x_rays_another_players_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DR_PEEKER),
            ("Bob", &HENCHMAN),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Fellow Super Villains: Bob"));
    assert!(report.dms_to("Alice").contains("Carol holds Seer"));
    let bob_dms = report.dms_to("Bob");
    assert!(bob_dms.contains("Syndicate Update:"));
    assert!(bob_dms.contains("X-rayed Carol's card!"));
    assert!(!bob_dms.contains("Seer"));
}

#[tokio::test(start_paused = true)]
async fn dr_peeker_prompts_after_temptress_syndicate_update_and_keeps_face_private() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &DR_PEEKER),
            ("Carol", &SEER),
            ("Dave", &DETECTOR),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Bob", "Dave").await;
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Dave", "pass").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    let temptress_update_pos = bob_dms
        .find("planted the extra Villain card onto Carol")
        .expect("Dr. Peeker must receive Temptress's Syndicate Update");
    let peeker_prompt_pos = bob_dms
        .find("Dr. Peeker: Reply")
        .expect("Dr. Peeker must be prompted");
    assert!(
        temptress_update_pos < peeker_prompt_pos,
        "Dr. Peeker should receive Temptress's target update before choosing whom to X-ray:\n{bob_dms}"
    );
    assert!(bob_dms.contains("Dave holds Detector"));

    let alice_dms = report.dms_to("Alice");
    assert!(alice_dms.contains("X-rayed Dave's card!"));
    assert!(!alice_dms.contains("Detector"));
    assert!(!report.dms_to("Carol").contains("Syndicate Update"));
}

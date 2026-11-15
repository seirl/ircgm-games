use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::curator::CURATOR;
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

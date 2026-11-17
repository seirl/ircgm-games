use crate::roles::alien::empath::{EmpathQuestion, ForcedEmpathQuestion, EMPATH};
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn empath_observes_closed_eye_table_poll_across_all_other_players() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER),
            ("Bob", &EMPATH),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedEmpathQuestion(Some(EmpathQuestion::MostSuspicious)));

    let night = h.spawn_night();
    h.reply_dm("Carol", "1").await;
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Carol").await;
    h.wait_for_prompt_dm("Dave").await;
    h.send_dm("Alice", "Carol").await;
    h.send_dm("Carol", "Alice").await;
    h.send_dm("Dave", "Carol").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    assert!(
        bob_dms.contains("Alice → Carol, Carol → Alice, Dave → Carol"),
        "got:\n{bob_dms}"
    );
}

use crate::marks::Mark;
use crate::roles::base::{seer::SEER, villager::VILLAGER};
use crate::roles::vampire::{count::COUNT, renfield::RENFIELD, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn vampire_coven_rejects_talking_relays_silent_points_and_infects_thrall() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &COUNT),
            ("Carol", &RENFIELD),
            ("Dave", &SEER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Bob").await;

    h.send_dm("Alice", "let's turn Eve and fear Dave?").await;
    let alice_rej = h.wait_for_new_dm("Alice").await;
    assert!(alice_rej.contains("Silent night"), "got: {alice_rej}");

    h.send_dm("Alice", "Dave").await;
    h.send_dm("Bob", "Eve").await;
    h.reply_dm("Bob", "Dave").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    assert!(!bob_dms.contains("let's turn Eve"));
    assert!(bob_dms.contains("Alice points at Dave!"));
    assert!(report.dms_to("Alice").contains("Bob points at Eve!"));
    assert_eq!(report.mark_at(&h.roster, "Eve"), Mark::Vampire);
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Bat);
    assert_eq!(report.mark_at(&h.roster, "Dave"), Mark::Fear);
    assert!(report.has_chronicle_substr(
        "Vampire Coven (Alice, Bob) bestowed Mark of the Vampire on Eve."
    ));
}

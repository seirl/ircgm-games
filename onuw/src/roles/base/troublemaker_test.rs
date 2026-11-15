use crate::roles::base::{
    seer::SEER, troublemaker::TROUBLEMAKER, villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn troublemaker_swaps_two_other_players_and_rejects_self() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &TROUBLEMAKER),
            ("Bob", &WEREWOLF),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Carol", "pass").await;
    // Reject swapping self:
    h.reply_dm("Alice", "Alice Bob").await;
    h.send_dm("Alice", "Bob Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Seer");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert!(report
        .has_chronicle_substr("Troublemaker (Alice) swapped Bob (Werewolf) and Carol (Seer)."));
}

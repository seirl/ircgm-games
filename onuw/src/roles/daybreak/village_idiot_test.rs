use crate::roles::base::{seer::SEER, tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::{sentinel::SENTINEL, village_idiot::VILLAGE_IDIOT};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn village_idiot_rotates_unshielded_other_cards_skipping_self_and_shielded_seats() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGE_IDIOT),
            ("Bob", &SENTINEL),
            ("Carol", &WEREWOLF),
            ("Dave", &SEER),
            ("Eve", &TANNER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Sentinel Bob shields Carol (Werewolf):
    h.reply_dm("Bob", "Carol").await;
    // Lone Wolf Carol passes:
    h.reply_dm("Carol", "pass").await;
    // Seer Dave passes:
    h.reply_dm("Dave", "pass").await;
    // Village Idiot Alice rotates other unshielded cards {Bob, Dave, Eve} RIGHT (+1):
    // Bob <- Eve (Tanner), Dave <- Bob (Sentinel), Eve <- Dave (Seer), while Alice stays Village Idiot and shielded Carol stays Werewolf!
    h.reply_dm("Alice", "right").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Village Idiot");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Tanner");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Sentinel");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Seer");
}

use crate::roles::alien::rascal::{ForcedRascalMode, RascalMode, RASCAL};
use crate::roles::base::{tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn rascal_robber_and_witch_modes_move_and_view_cards_cleanly() {
    // 1. Robber mode:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &RASCAL),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedRascalMode(Some(RascalMode::Robber)));
    let night = h.spawn_night();
    h.reply_dm("Bob", "1").await;
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("your new role is Werewolf"));
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Rascal");

    // 2. Witch mode:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &RASCAL),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&TANNER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedRascalMode(Some(RascalMode::Witch)));
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "1").await;
    h2.reply_dm("Alice", "Bob").await;
    let rep2 = h2.finish_night(n2).await;
    assert_eq!(rep2.effective_role_at(&h2.roster, "Bob"), "Tanner");
}

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

    // 3. Rascal + Doppelgänger-Rascal run sequentially in Witch mode (second actor sees live Center #1),
    //    and unheld Rascal in center still emits the Mother Ship public announcement:
    use crate::roles::base::doppelganger::DOPPELGANGER;
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &RASCAL),
            ("Bob", &DOPPELGANGER),
            ("Carol", &VILLAGER),
        ],
        [&TANNER, &WEREWOLF, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedRascalMode(Some(RascalMode::Witch)));
    let n3 = h3.spawn_night();
    h3.reply_dm("Bob", "Alice").await; // Doppelgänger copies Rascal
    // Slot 1 (#7-F): Alice peeks Center #1 (Tanner) and swaps it onto Carol (putting Villager in Center #1)
    h3.reply_dm("Alice", "1").await;
    h3.reply_dm("Alice", "Carol").await;
    // Slot 2 (#7-F-A): Bob peeks Center #1 — sees Carol's old Villager card (not stale Tanner!) and swaps onto Alice
    h3.reply_dm("Bob", "1").await;
    h3.reply_dm("Bob", "Alice").await;
    let rep3 = h3.finish_night(n3).await;
    let bob_plain = ircgm::prelude::Text::parse(&rep3.dms_to("Bob")).plain();
    assert!(bob_plain.contains("Center #1 is Villager!"), "got:\n{bob_plain}");
    assert!(!bob_plain.contains("Tanner"));
    assert_eq!(rep3.effective_role_at(&h3.roster, "Carol"), "Tanner");
    assert_eq!(rep3.effective_role_at(&h3.roster, "Alice"), "Villager");

    let mut h4 = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&RASCAL, &VILLAGER, &VILLAGER],
    )
    .await;
    h4.set_key(ForcedRascalMode(Some(RascalMode::Idiot)));
    let n4 = h4.spawn_night();
    let rep4 = h4.finish_night(n4).await;
    assert!(rep4
        .channel_lines
        .iter()
        .any(|l| l.contains("Mother Ship (Rascal)")));
}

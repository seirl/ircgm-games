use crate::board::CenterSlot;
use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    exposer::{ForcedExposerCount, EXPOSER},
};
use crate::roles::base::{tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::curator::CURATOR;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn exposer_all_or_nothing_flips_center_cards_visible_to_later_wakers() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &EXPOSER),
            ("Bob", &CURATOR),
            ("Carol", &ALIEN),
            ("Dave", &VILLAGER),
        ],
        [&WEREWOLF, &TANNER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    h.set_key(ForcedExposerCount(Some(2)));

    let night = h.spawn_night();
    h.wait_for_prompt_dm("Alice").await;
    h.send_dm("Alice", "1").await;
    let rej = h.wait_for_new_dm("Alice").await;
    assert!(rej.contains("EXACTLY 2"), "got: {rej}");
    h.send_dm("Alice", "1 2").await;
    h.reply_dm("Bob", "pass").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    assert!(
        bob_dms.contains("Center #1 [🔦 Werewolf]") && bob_dms.contains("Center #2 [🔦 Tanner]"),
        "expected Curator to see exposed center cards, got:\n{bob_dms}"
    );
    assert!(report.board.is_center_face_up(CenterSlot::Main(0)));
    assert!(report.board.is_center_face_up(CenterSlot::Main(1)));
    assert!(!report.board.is_center_face_up(CenterSlot::Main(2)));

    // Exposer + Doppelgänger-Exposer run sequentially: Doppelgänger sees Center #1 & #2 already exposed
    // and can only expose remaining face-down Center #3, while Oracle logs them separately:
    use crate::roles::alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE};
    use crate::roles::base::doppelganger::DOPPELGANGER;
    use ircgm::prelude::Text;

    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &EXPOSER),
            ("Bob", &DOPPELGANGER),
            ("Carol", &ORACLE),
            ("Dave", &VILLAGER),
        ],
        [&WEREWOLF, &TANNER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 4,
    })));
    h2.set_key(ForcedExposerCount(Some(2)));
    let n2 = h2.spawn_night();
    h2.reply_dm("Carol", "4").await; // Oracle jackpot (All-Night Watch)
    h2.reply_dm("Bob", "Alice").await; // Doppelgänger -> Exposer
    h2.reply_dm("Alice", "1 2").await; // Exposer flips #1 and #2 face up
    h2.wait_for_prompt_dm("Bob").await;
    h2.send_dm("Bob", "1").await; // Rejected: #1 is already face up!
    let rej_bob = h2.wait_for_new_dm("Bob").await;
    assert!(rej_bob.contains("already face up"), "got: {rej_bob}");
    h2.send_dm("Bob", "3").await; // Clamped to k = 1 remaining face-down center card
    let rep2 = h2.finish_night(n2).await;
    assert!(rep2.board.is_center_face_up(CenterSlot::Main(0)));
    assert!(rep2.board.is_center_face_up(CenterSlot::Main(1)));
    assert!(rep2.board.is_center_face_up(CenterSlot::Main(2)));
    let carol_watch = Text::parse(&rep2.dms_to("Carol")).plain();
    assert!(
        carol_watch.contains("Exposer (#1/2): Alice · Doppelgänger-Exposer: Bob"),
        "got:\n{carol_watch}"
    );
}

use ircgm::prelude::Text;

use crate::roles::alien::psychic::{ForcedPsychicMode, PsychicMode, PSYCHIC};
use crate::roles::base::{seer::SEER, tanner::TANNER, villager::VILLAGER};
use crate::roles::daybreak::sentinel::SENTINEL;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn psychic_both_neighbors_and_even_seat_modes_read_minds_and_show_visible_table() {
    // 1. BothNeighbors mode + visible table shield marker:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SENTINEL),
            ("Bob", &SEER),
            ("Carol", &PSYCHIC),
            ("Dave", &TANNER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedPsychicMode(Some(PsychicMode::BothNeighbors)));
    let night = h.spawn_night();
    h.reply_dm("Alice", "Dave").await;
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;
    let carol_plain = Text::parse(&report.dms_to("Carol")).plain();
    assert!(carol_plain.contains("Dave🛡"));
    assert!(carol_plain.contains("Bob holds Seer"));
    assert!(!carol_plain.contains("Dave holds Tanner"));

    // 2. EvenSeat mode:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER),
            ("Bob", &TANNER),
            ("Carol", &VILLAGER),
            ("Dave", &PSYCHIC),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedPsychicMode(Some(PsychicMode::EvenSeat)));
    let n2 = h2.spawn_night();
    h2.reply_dm("Dave", "Bob").await;
    let rep2 = h2.finish_night(n2).await;
    assert!(rep2.dms_to("Dave").contains("Bob holds Tanner"));
}

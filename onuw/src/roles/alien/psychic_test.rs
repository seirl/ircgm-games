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

    // 3. Timed-out Psychic views nothing (does NOT trigger Aura Seer), and 3p games never roll EvenSeat
    //    regardless of which seat (or center) holds Psychic:
    use crate::roles::daybreak::aura_seer::AURA_SEER;
    for seat2_psychic in [true, false] {
        for _ in 0..15 {
            let seats: &[(&str, &'static dyn crate::role::Role)] = if seat2_psychic {
                &[("Alice", &AURA_SEER), ("Bob", &PSYCHIC), ("Carol", &VILLAGER)]
            } else {
                &[("Alice", &PSYCHIC), ("Bob", &VILLAGER), ("Carol", &VILLAGER)]
            };
            let mut h3 =
                OnuwTestHarness::new(seats, [&VILLAGER, &VILLAGER, &VILLAGER]).await;
            let n3 = h3.spawn_night();
            let rep3 = h3.finish_night(n3).await;
            assert!(!rep3
                .channel_lines
                .join("\n")
                .contains("EVEN-numbered player's card"));
        }
    }

    let mut h_timeout = OnuwTestHarness::new(
        &[
            ("Alice", &AURA_SEER),
            ("Bob", &PSYCHIC),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_timeout.set_key(ForcedPsychicMode(Some(PsychicMode::ChooseNeighbor)));
    let nt = h_timeout.spawn_night();
    let rept = h_timeout.finish_night(nt).await;
    assert!(rept
        .dms_to("Alice")
        .contains("Nobody else viewed or moved any cards"));
}

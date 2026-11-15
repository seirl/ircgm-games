use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::revealer::REVEALER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn revealer_keeps_village_face_up_but_flips_werewolf_back_face_down() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &REVEALER),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(!report.board.is_player_face_up(h.seat_of("Bob")));
    assert!(report.dms_to("Alice").contains("flip it BACK face down!"));
    assert!(report
        .has_chronicle_substr("Revealer (Alice) peeked Bob (Werewolf) and flipped it back face-down."));
}

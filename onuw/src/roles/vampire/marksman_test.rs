use crate::roles::base::{doppelganger::DOPPELGANGER, villager::VILLAGER};
use crate::roles::vampire::{marksman::MARKSMAN, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn marksman_and_doppelganger_marksman_inspect_card_and_dusk_mark() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MARKSMAN),
            ("Carol", &VAMPIRE),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Carol", "Dave").await;
    h.reply_dm("Bob", "Carol Dave").await;
    h.reply_dm("Alice", "Dave Carol").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Bob")
        .contains("Carol's card is Vampire, and Dave's mark is Mark of the Vampire!"));
    assert!(report
        .dms_to("Alice")
        .contains("Dave's card is Villager, and Carol's mark is Mark of Clarity!"));
}

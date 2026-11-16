use crate::marks::Mark;
use crate::roles::base::villager::VILLAGER;
use crate::roles::vampire::{renfield::RENFIELD, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn renfield_starts_with_bat_mark_and_sees_vampires_and_marked_thrall() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &VILLAGER),
            ("Carol", &RENFIELD),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Vampire) places Mark::Vampire on Bob:
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Bat);
    assert!(report
        .dms_to("Carol")
        .contains("Vampires: Alice · Marked with Mark of the Vampire: Bob"));
}

#[tokio::test(start_paused = true)]
async fn renfield_sees_message_when_no_vampires_are_in_play() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &RENFIELD),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VAMPIRE, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    let report = h.finish_night(night).await;

    assert_eq!(report.mark_at(&h.roster, "Alice"), Mark::Bat);
    assert!(report
        .dms_to("Alice")
        .contains("No players are Vampires tonight!"));

    // With 0 Vampires in play, Renfield is on Team::Village and wins with Village when nobody dies!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Carol"]);
}

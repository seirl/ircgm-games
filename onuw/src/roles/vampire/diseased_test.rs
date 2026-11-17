use crate::marks::Mark;
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::vampire::diseased::DISEASED;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn diseased_and_mark_of_disease_strip_victory_from_voters() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DISEASED),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice places Mark::Disease on right neighbor Bob:
    h.reply_dm("Alice", "right").await;
    h.reply_dm("Carol", "1").await;
    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Disease);

    // Carol (Werewolf) dies so Village wins, EXCEPT Alice voted Bob (Mark::Disease) and Dave voted Alice (Diseased)!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Carol")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob"]);
}

use crate::marks::Mark;
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::vampire::instigator::INSTIGATOR;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn instigator_mark_of_traitor_wins_only_when_teammate_dies() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &INSTIGATOR),
            ("Bob", &WEREWOLF),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice places Mark::Traitor on Werewolf Bob:
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Traitor);

    // Carol (fellow Werewolf) dies -> Village wins AND Traitor Bob wins!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Dave"]);
}

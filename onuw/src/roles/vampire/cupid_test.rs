use crate::marks::Mark;
use crate::roles::base::{hunter::HUNTER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::vampire::cupid::CUPID;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn cupid_love_heartbreak_chains_into_hunter_death_shot() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CUPID),
            ("Bob", &HUNTER),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Alice Bob").await;
    h.reply_dm("Carol", "1").await;
    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Alice"), Mark::Love);
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Love);

    // Alice dies by vote -> Lover Bob (Hunter) dies of heartbreak -> Hunter shoots Werewolf Carol!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    let dead = dawn.dead_nicks(&h.roster);
    assert!(dead.contains("Alice"));
    assert!(dead.contains("Bob"));
    assert!(dead.contains("Carol"));
    assert_eq!(
        dawn.winner_nicks(&h.roster),
        vec!["Alice", "Bob", "Dave"]
    );
}

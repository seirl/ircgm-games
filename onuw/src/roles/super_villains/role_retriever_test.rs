use crate::roles::base::{
    robber::ROBBER, troublemaker::TROUBLEMAKER, villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::roles::super_villains::role_retriever::ROLE_RETRIEVER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn role_retriever_runs_after_robber_and_dawn_withholds_chronicle_from_channel() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ROBBER),
            ("Bob", &ROLE_RETRIEVER),
            ("Carol", &TROUBLEMAKER),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Dave", "pass").await;
    h.reply_dm("Alice", "Dave").await;
    h.reply_dm("Bob", "Alice").await;
    h.reply_dm("Carol", "Bob Dave").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Role Retriever");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Robber");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Werewolf");
    assert!(!report.dms_to("Alice").contains("Role Retriever swapped"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    let chan = dawn.channel_text();
    assert!(!chan.contains("📜 What Happened:"));
    let reveal = dawn.reveal_text();
    assert!(reveal.contains(
        "Role Retriever (Bob) fetched Werewolf from Alice (giving Alice Role Retriever)."
    ));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Carol"]);
}

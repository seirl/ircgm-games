use crate::roles::alien::mortician::{ForcedMorticianMode, MorticianMode, MORTICIAN};
use crate::roles::base::{seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn mortician_peeks_neighbors_and_wins_when_surviving_with_dead_neighbor() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MORTICIAN),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedMorticianMode(Some(MorticianMode::BothNeighbors)));

    let night = h.spawn_night();
    h.reply_dm("Bob", "1").await;
    h.reply_dm("Dave", "pass").await;
    h.reply_dm("Alice", "both").await;

    let report = h.finish_night(night).await;
    let alice_dms = report.dms_to("Alice");
    assert!(alice_dms.contains("Bob holds Werewolf"));
    assert!(alice_dms.contains("Dave holds Seer"));

    // Adjacent neighbor Bob dies while Mortician Alice survives -> Alice co-wins alongside Village!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Carol", "Dave"]);
}

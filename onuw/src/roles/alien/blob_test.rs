use ircgm::prelude::Text;

use crate::roles::alien::blob::{ActiveBlobShape, BlobShape, BLOB};
use crate::roles::base::{drunk::DRUNK, seer::SEER, villager::VILLAGER};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn blob_announces_shape_even_when_in_center_and_wins_when_absorbed_cluster_survives() {
    let mut h = OnuwTestHarness::new(
        &[("Alice", &VILLAGER), ("Bob", &SEER), ("Carol", &DRUNK)],
        [&BLOB, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ActiveBlobShape(Some(BlobShape::Right1)));

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Carol", "1").await; // Drunk Carol swaps into Center #1 (Blob)!
    let rep = h.finish_night(night).await;

    assert!(rep
        .channel_lines
        .iter()
        .any(|l| Text::parse(l).plain().contains("1 player to their RIGHT")));

    // Bob dies; Carol (#3, Blob) and her right neighbor Alice (#1) both survive -> Carol wins solo!
    let dawn = h
        .resolve_voting(
            rep,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Carol"]);
}

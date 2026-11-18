use ircgm::prelude::Text;

use crate::board::SeatSet;
use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    blob::{ActiveBlobShape, BlobShape, BLOB},
};
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

#[tokio::test(start_paused = true)]
async fn blob_absorption_count_scales_with_player_count_from_3p_to_10p() {
    // Official ONUA absorption count (total cluster = 1 + other absorbed neighbors):
    // 3p -> 1 (0 others), 4p -> 2 (1 other), 5-6p -> 3 (2 others), 7-8p -> 4 (3 others), 9-10p -> 5 (4 others).
    let expected_total = |n: usize| match n {
        3 => 1,
        4 => 2,
        5 | 6 => 3,
        7 | 8 => 4,
        9 | 10 => 5,
        _ => unreachable!(),
    };

    for n in 3..=10 {
        for _ in 0..40 {
            let shape = BlobShape::roll(n);
            for seat in 0..n {
                let absorbed = shape.absorbed_seats(n, seat);
                assert!(absorbed.contains(seat));
                assert_eq!(
                    absorbed.len(),
                    expected_total(n),
                    "n={n}, seat={seat}, shape={shape:?}"
                );
            }
        }
    }

    // Check wrap-around geometry at seat 0 for 9-player shapes:
    assert_eq!(
        BlobShape::Left4.absorbed_seats(9, 0),
        SeatSet::from([0, 8, 7, 6, 5])
    );
    assert_eq!(
        BlobShape::Right4.absorbed_seats(9, 0),
        SeatSet::from([0, 1, 2, 3, 4])
    );
    assert_eq!(
        BlobShape::Left2Right2.absorbed_seats(9, 0),
        SeatSet::from([7, 8, 0, 1, 2])
    );
    assert_eq!(
        BlobShape::Left3.absorbed_seats(7, 0),
        SeatSet::from([0, 6, 5, 4])
    );
    assert_eq!(
        BlobShape::Right3.absorbed_seats(7, 6),
        SeatSet::from([6, 0, 1, 2])
    );

    // Full 3p game: Blob rolls `SelfOnly` (0 other players), so executing adjacent Alien Bob
    // lets Blob Alice co-win alongside Villager Carol!
    let mut h3 = OnuwTestHarness::new(
        &[("Alice", &BLOB), ("Bob", &ALIEN), ("Carol", &VILLAGER)],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n3 = h3.spawn_night();
    let rep3 = h3.finish_night(n3).await;
    assert!(rep3
        .channel_lines
        .iter()
        .any(|l| Text::parse(l).plain().contains("0 other players")));

    let dawn3 = h3
        .resolve_voting(
            rep3,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Alice", "Carol"]);
}

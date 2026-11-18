use ircgm::prelude::Text;

use crate::roles::alien::mortician::{ForcedMorticianMode, MorticianMode, MORTICIAN};
use crate::roles::base::{seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn mortician_peeks_neighbors_and_wins_when_neighbor_dies_even_if_mortician_also_dies() {
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

    // 1. Adjacent neighbor Bob dies while Mortician Alice survives -> Alice co-wins alongside Village!
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

    // 2. MorticianMode::None denies peeking, records zero card views, and Mortician STILL wins when
    //    dying alongside adjacent neighbor Bob on a 2-2 vote tie!
    let mut h_tie = OnuwTestHarness::new(
        &[
            ("Alice", &MORTICIAN),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_tie.set_key(ForcedMorticianMode(Some(MorticianMode::None)));
    let n_tie = h_tie.spawn_night();
    h_tie.reply_dm("Bob", "pass").await;
    let rep_tie = h_tie.finish_night(n_tie).await;

    let alice_none = Text::parse(&rep_tie.dms_to("Alice")).plain();
    assert!(alice_none.contains("NONE of your neighbors' cards"));
    assert!(!alice_none.contains("Mortician Peek:"));
    assert!(rep_tie
        .channel_lines
        .iter()
        .any(|l| Text::parse(l)
            .plain()
            .contains("peek at NONE of their neighbors' cards")));
    assert!(format!("{:?}", rep_tie.board)
        .contains(&format!("viewed_or_moved_actors: {:?}", crate::board::SeatSet::EMPTY)));

    // 2-2 tie kills BOTH Mortician Alice and neighbor Bob -> Alice co-wins alongside Village!
    let dawn_tie = h_tie
        .resolve_voting(
            rep_tie,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_tie.winner_nicks(&h_tie.roster),
        vec!["Alice", "Carol", "Dave"]
    );

    // 3. Mortician dying ALONE (both immediate neighbors survive) does NOT win:
    let mut h_alone = OnuwTestHarness::new(
        &[
            ("Alice", &MORTICIAN),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_alone.set_key(ForcedMorticianMode(Some(MorticianMode::None)));
    let n_alone = h_alone.spawn_night();
    h_alone.reply_dm("Carol", "pass").await;
    let rep_alone = h_alone.finish_night(n_alone).await;
    let dawn_alone = h_alone
        .resolve_voting(
            rep_alone,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn_alone.winner_nicks(&h_alone.roster), vec!["Carol"]);
}

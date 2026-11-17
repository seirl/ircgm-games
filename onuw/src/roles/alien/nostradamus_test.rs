use ircgm::prelude::Text;

use crate::role::Team;
use crate::roles::alien::nostradamus::NOSTRADAMUS;
use crate::roles::base::{
    doppelganger::DOPPELGANGER, drunk::DRUNK, robber::ROBBER, tanner::TANNER, villager::VILLAGER,
    werewolf::WEREWOLF,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn nostradamus_stops_on_non_village_announces_team_and_must_survive_to_win() {
    // 1. Nostradamus sees Villager then Tanner -> stops, announces Tanner, and co-wins when Tanner dies & Nostradamus lives!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &VILLAGER),
            ("Carol", &TANNER),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Dave", "1").await;
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;
    let report = h.finish_night(night).await;

    assert!(report.channel_lines.iter().any(|l| {
        let plain = Text::parse(l).plain();
        plain.contains("Nostradamus Prophecy: Nostradamus has joined Tanner!")
    }));

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
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Carol"]);

    // 2. Nostradamus joins Werewolf team on peek #1 (stopping immediately!), then dies while Werewolf survives:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &WEREWOLF),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Bob", "1").await;
    h2.reply_dm("Alice", "Bob").await;
    let report2 = h2.finish_night(night2).await;

    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Bob"]);
}

#[tokio::test(start_paused = true)]
async fn doppelganger_nostradamus_emits_two_separate_prophecies_tracks_card_swaps_and_bluffs_when_uncopied()
{
    // 1. Doppelgänger (Alice) copies Nostradamus (Bob): both wake, pick DIFFERENT destinies
    //    (Bob -> Tanner, Alice -> Werewolf Team), emit TWO separate named prophecies BEFORE Robber (#6) acts,
    //    and when Eve (Robber) steals Bob's Nostradamus card, Eve inherits Bob's Tanner destiny while Alice keeps Werewolf Team!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &NOSTRADAMUS),
            ("Carol", &TANNER),
            ("Dave", &WEREWOLF),
            ("Eve", &ROBBER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Dave", "1").await;
    h.wait_for_prompt_dm("Bob").await;
    h.wait_for_prompt_dm("Alice").await;
    h.send_dm("Bob", "Carol").await;
    h.send_dm("Alice", "Dave").await;
    h.reply_dm("Eve", "Bob").await;
    let report = h.finish_night(night).await;

    let plain_lines: Vec<String> = report
        .channel_lines
        .iter()
        .map(|l| Text::parse(l).plain())
        .collect();
    let nostra_idx = plain_lines
        .iter()
        .position(|l| l.contains("Nostradamus Prophecy: Nostradamus has joined Tanner!"))
        .expect("separate Nostradamus prophecy");
    let dg_nostra_idx = plain_lines
        .iter()
        .position(|l| {
            l.contains(
                "Nostradamus Prophecy: Doppelgänger-Nostradamus has joined Werewolf Team!",
            )
        })
        .expect("separate Doppelgänger-Nostradamus prophecy");
    let robber_tier_idx = plain_lines
        .iter()
        .position(|l| l.contains("Calling: Robber"))
        .expect("Robber called");
    assert!(nostra_idx < dg_nostra_idx);
    assert!(dg_nostra_idx < robber_tier_idx);

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
                ("Eve", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Carol", "Eve"]);

    // 2. When Doppelgänger is in the deck but copies Villager (NOT Nostradamus), BOTH separate prophecies
    //    are STILL announced so Doppelgänger's target never leaks, while Alice stays on Team::Village!
    let mut h_decoy = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &NOSTRADAMUS),
            ("Carol", &TANNER),
            ("Dave", &VILLAGER),
        ],
        [&WEREWOLF, &WEREWOLF, &WEREWOLF],
    )
    .await;
    let n_decoy = h_decoy.spawn_night();
    h_decoy.reply_dm("Alice", "Dave").await;
    h_decoy.reply_dm("Bob", "Carol").await;
    let rep_decoy = h_decoy.finish_night(n_decoy).await;

    let decoy_lines: Vec<String> = rep_decoy
        .channel_lines
        .iter()
        .map(|l| Text::parse(l).plain())
        .collect();
    assert!(decoy_lines
        .iter()
        .any(|l| l.contains("Nostradamus Prophecy: Nostradamus has joined Tanner!")));
    assert!(decoy_lines
        .iter()
        .any(|l| l.contains("Nostradamus Prophecy: Doppelgänger-Nostradamus has joined ")));
    assert_eq!(rep_decoy.board.effective_team(0), Team::Village);

    // 3. Unheld center Nostradamus: announces a random deck team at #5E AND Drunk (#8) taking Center #1 inherits it!
    let mut h_center = OnuwTestHarness::new(
        &[("Alice", &DRUNK), ("Bob", &WEREWOLF), ("Carol", &VILLAGER)],
        [&NOSTRADAMUS, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night_c = h_center.spawn_night();
    h_center.reply_dm("Bob", "2").await;
    h_center.reply_dm("Alice", "1").await;
    let rep_c = h_center.finish_night(night_c).await;

    let center_prophecy = rep_c
        .channel_lines
        .iter()
        .map(|l| Text::parse(l).plain())
        .find(|p| p.contains("Nostradamus Prophecy:"))
        .expect("center Nostradamus still announces prophecy");
    let expected_team = if center_prophecy.contains("Werewolf Team") {
        Team::Werewolf
    } else {
        Team::Village
    };
    assert_eq!(rep_c.board.effective_team(0), expected_team);

    // 4. Two Doppelgängers (`doppelganger*2`) BOTH copy Nostradamus: one Doppelgänger-Nostradamus is drawn
    //    at random to peek, the other is told another Doppelgänger-Nostradamus is gazing, and BOTH share that team!
    let mut h_2dg = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &DOPPELGANGER),
            ("Carol", &NOSTRADAMUS),
            ("Dave", &TANNER),
            ("Eve", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_2dg = h_2dg.spawn_night();
    h_2dg.wait_for_prompt_dm("Alice").await;
    h_2dg.wait_for_prompt_dm("Bob").await;
    h_2dg.send_dm("Alice", "Carol").await;
    h_2dg.send_dm("Bob", "Carol").await;
    h_2dg.reply_dm("Eve", "1").await;

    // Carol (Nostradamus) peeks Eve (Werewolf), while ONE of Alice/Bob peeks Dave (Tanner):
    h_2dg.wait_for_prompt_dm("Carol").await;
    let a_dm = h_2dg.wait_for_new_dm("Alice").await;
    let b_dm = h_2dg.wait_for_new_dm("Bob").await;
    let dg_seer = if a_dm.contains("Reply") {
        assert!(b_dm.contains("Another Doppelgänger-Nostradamus is gazing"));
        "Alice"
    } else {
        assert!(a_dm.contains("Another Doppelgänger-Nostradamus is gazing"));
        "Bob"
    };
    h_2dg.send_dm("Carol", "Eve").await;
    h_2dg.send_dm(dg_seer, "Dave").await;
    let rep_2dg = h_2dg.finish_night(n_2dg).await;

    let dawn_2dg = h_2dg
        .resolve_voting(
            rep_2dg,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Eve")),
                ("Eve", Some("Dave")),
            ],
        )
        .await;
    // Dave (Tanner) dies -> BOTH Alice and Bob (Doppelgänger-Nostradamus -> Tanner) co-win with Dave,
    // while Carol (Nostradamus -> Werewolf Team, blocked by Tanner) does not!
    assert_eq!(
        dawn_2dg.winner_nicks(&h_2dg.roster),
        vec!["Alice", "Bob", "Dave"]
    );
}

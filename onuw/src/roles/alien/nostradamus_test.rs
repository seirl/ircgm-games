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

#[tokio::test(start_paused = true)]
async fn feared_doppelganger_nostradamus_adopts_announced_decoy_and_monster_follower_wins_with_zero_wolves()
{
    use crate::roles::base::minion::MINION;
    use crate::roles::vampire::count::COUNT;

    // 1. Doppelgänger copies Nostradamus at #-7, then Count paralyzes Doppelgänger with Mark::Fear at #-6B:
    //    Doppelgänger-Nostradamus adopts the publicly announced random destiny instead of cloning Nostradamus's!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &NOSTRADAMUS),
            ("Carol", &COUNT),
            ("Dave", &TANNER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;
    // Count wakes at #-6 (Vampire coven mark) and then at #-6B (Mark of Fear on Alice):
    h.reply_dm("Carol", "Dave").await;
    h.reply_dm("Carol", "Alice").await;
    h.reply_dm("Bob", "Dave").await;
    let rep = h.finish_night(n).await;

    let dg_prophecy = rep
        .channel_lines
        .iter()
        .map(|l| Text::parse(l).plain())
        .find(|p| p.contains("Doppelgänger-Nostradamus has joined "))
        .expect("Doppelgänger-Nostradamus prophecy announced");
    let expected = if dg_prophecy.contains("Vampire Team") {
        Team::Vampire
    } else if dg_prophecy.contains("Tanner") {
        Team::Independent
    } else {
        Team::Village
    };
    assert_eq!(rep.board.effective_team(0), expected);

    // 2. Nostradamus sees Minion (joining Werewolf Team immediately) with 0 Werewolves in play:
    //    when Villager Carol dies, BOTH surviving Minion AND surviving Nostradamus win!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &MINION),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    let rep2 = h2.finish_night(n2).await;
    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Alice", "Bob"]);

    // 3. Viewing a solo Groob (with Zerb in the center) announces "Alien Team!", never leaking "Groob"!
    use crate::roles::alien::{
        alien::{AlienEvent, ForcedAlienEvent},
        groob_zerb::{GROOB, ZERB},
    };
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &GROOB),
            ("Carol", &VILLAGER),
        ],
        [&ZERB, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n3 = h3.spawn_night();
    h3.reply_dm("Alice", "Bob").await;
    let rep3 = h3.finish_night(n3).await;
    let plain3 = rep3
        .channel_lines
        .iter()
        .map(|l| Text::parse(l).plain())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain3.contains("Nostradamus has joined Alien Team!"));
    assert!(!plain3.contains("joined Groob"));
}

#[tokio::test(start_paused = true)]
async fn nostradamus_evaluates_adopted_independent_victory_when_card_moves_to_center_or_holder_is_converted()
{
    use crate::artifacts::Artifact;
    use crate::marks::Mark;
    use crate::roles::{
        alien::mortician::{ForcedMorticianMode, MorticianMode, MORTICIAN},
        daybreak::witch::WITCH,
        super_villains::family_man::{ActiveFamilyShape, FamilyShape, FAMILY_MAN},
        vampire::{
            apprentice_tanner::APPRENTICE_TANNER, assassin::ASSASSIN, vampire::VAMPIRE,
        },
    };

    // 1. Family Man swapped to center by Witch (#6-B) after Nostradamus peeks it at #5-E:
    //    no player holds Family Man at Voting, so family survival evaluates relative to Nostradamus (Alice)!
    let mut h_fam = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &FAMILY_MAN),
            ("Carol", &WITCH),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_fam.set_key(ActiveFamilyShape(Some(FamilyShape::Right1)));
    let n_fam = h_fam.spawn_night();
    h_fam.reply_dm("Dave", "1").await;
    h_fam.reply_dm("Alice", "Bob").await;
    // Witch gives Center #1 (Villager) to Bob, pushing Family Man into Center #1!
    h_fam.reply_dm("Carol", "1").await;
    h_fam.reply_dm("Carol", "Bob").await;
    let rep_fam = h_fam.finish_night(n_fam).await;

    // Dave (seat 3, NOT Alice's Right1 neighbor Bob) dies -> Village wins AND Alice (Nostradamus -> Family Man) co-wins!
    let dawn_fam = h_fam
        .resolve_voting(
            rep_fam,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(
        dawn_fam.winner_nicks(&h_fam.roster),
        vec!["Alice", "Bob", "Carol"]
    );

    // 2. Family Man converted by Mark::Vampire in Dusk (card stays at Bob's seat 2):
    //    family survival evaluates relative to the Family Man card's seat (Bob, seat 2 -> Right1 is Dave, seat 3)!
    let mut h_vamp_fam = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Carol", &VAMPIRE),
            ("Bob", &FAMILY_MAN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_vamp_fam.set_key(ActiveFamilyShape(Some(FamilyShape::Right1)));
    let n_vf = h_vamp_fam.spawn_night();
    h_vamp_fam.reply_dm("Carol", "Bob").await; // Vampire bites Bob (Mark::Vampire)
    h_vamp_fam.reply_dm("Alice", "Bob").await; // Nostradamus peeks Bob's Family Man card face
    let rep_vf = h_vamp_fam.finish_night(n_vf).await;

    // 1-1-1-1 circle vote -> 0 deaths -> Vampires (Carol & Bob) win AND Alice (Nostradamus -> Family Man) co-wins!
    let dawn_vf = h_vamp_fam
        .resolve_voting(
            rep_vf,
            &[
                ("Alice", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Bob", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_vf.winner_nicks(&h_vamp_fam.roster),
        vec!["Alice", "Carol", "Bob"]
    );

    // 3. Assassin places Mark::Assassin on Dave in Dusk (#-1), Nostradamus peeks Assassin (Bob) at #5-E,
    //    and Witch (#6-B) swaps Bob's Assassin card into the center: when Dave dies, Nostradamus still wins!
    let mut h_ass = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &ASSASSIN),
            ("Carol", &WITCH),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_ass = h_ass.spawn_night();
    h_ass.reply_dm("Bob", "Dave").await;
    h_ass.reply_dm("Dave", "1").await;
    h_ass.reply_dm("Alice", "Bob").await;
    h_ass.reply_dm("Carol", "1").await;
    h_ass.reply_dm("Carol", "Bob").await;
    let rep_ass = h_ass.finish_night(n_ass).await;

    let dawn_ass = h_ass
        .resolve_voting(
            rep_ass,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(
        dawn_ass.winner_nicks(&h_ass.roster),
        vec!["Alice", "Bob", "Carol"]
    );

    // 4. Apprentice Tanner (Bob) swapped into center by Witch (#6-B) while Tanner (Dave) is in play and dies:
    //    surviving Nostradamus (Alice) still wins!
    let mut h_at = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &APPRENTICE_TANNER),
            ("Carol", &WITCH),
            ("Dave", &TANNER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_at = h_at.spawn_night();
    h_at.reply_dm("Alice", "Bob").await;
    h_at.reply_dm("Carol", "1").await;
    h_at.reply_dm("Carol", "Bob").await;
    let rep_at = h_at.finish_night(n_at).await;

    let dawn_at = h_at
        .resolve_voting(
            rep_at,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn_at.winner_nicks(&h_at.roster), vec!["Alice", "Dave"]);

    // 5. Tanner with Mark::Vampire is NOT a Tanner (0 active Tanners -> Nostradamus loses even when Bob dies),
    //    whereas if another seat holds CudgelOfTheTanner and dies, Nostradamus wins!
    let mut h_tan = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &TANNER),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_tan = h_tan.spawn_night();
    h_tan.reply_dm("Alice", "Bob").await;
    let mut rep_tan = h_tan.finish_night(n_tan).await;
    rep_tan.board.set_mark_for_test(1, Mark::Vampire);

    let dawn_tan_vamp = h_tan
        .resolve_voting(
            rep_tan,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    // Bob died as a Vampire (not a Tanner), so Village (Carol & Dave) wins and Nostradamus (Alice -> Tanner) loses!
    assert_eq!(
        dawn_tan_vamp.winner_nicks(&h_tan.roster),
        vec!["Carol", "Dave"]
    );

    let mut h_tan2 = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &TANNER),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_tan2 = h_tan2.spawn_night();
    h_tan2.reply_dm("Alice", "Bob").await;
    let mut rep_tan2 = h_tan2.finish_night(n_tan2).await;
    rep_tan2.board.set_mark_for_test(1, Mark::Vampire);
    rep_tan2
        .board
        .insert_artifact_for_test(2, Artifact::CudgelOfTheTanner);
    let dawn_tan_cudgel = h_tan2
        .resolve_voting(
            rep_tan2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    // Carol (Cudgel of the Tanner) dies -> Carol and surviving Nostradamus (Alice) win, blocking Vampire Bob!
    assert_eq!(
        dawn_tan_cudgel.winner_nicks(&h_tan2.roster),
        vec!["Alice", "Carol"]
    );

    // 6. Mortician swapped into center by Witch (#6-B): Nostradamus (Alice, seat 0) wins when adjacent neighbor Dave (seat 3) dies!
    let mut h_mort = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &MORTICIAN),
            ("Carol", &WITCH),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_mort.set_key(ForcedMorticianMode(Some(MorticianMode::LeftOnly)));
    let n_mort = h_mort.spawn_night();
    h_mort.reply_dm("Dave", "1").await;
    h_mort.reply_dm("Alice", "Bob").await;
    h_mort.reply_dm("Carol", "1").await;
    h_mort.reply_dm("Carol", "Bob").await;
    h_mort.reply_dm("Bob", "pass").await;
    let rep_mort = h_mort.finish_night(n_mort).await;

    let dawn_mort = h_mort
        .resolve_voting(
            rep_mort,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(
        dawn_mort.winner_nicks(&h_mort.roster),
        vec!["Alice", "Bob", "Carol"]
    );
}

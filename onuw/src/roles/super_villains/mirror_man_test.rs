use crate::roles::base::doppelganger::DOPPELGANGER;
use crate::roles::super_villains::{
    henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER, mirror_man::MIRROR_MAN,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn mirror_man_copies_center_role_and_stays_opaque_to_doppelganger() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &MIRROR_MAN),
            ("Bob", &DOPPELGANGER),
            ("Carol", &INNOCENT_BYSTANDER),
        ],
        [&HENCHMAN, &INNOCENT_BYSTANDER, &INNOCENT_BYSTANDER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "1").await;
    h.reply_dm("Bob", "Alice").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Bob").contains("Mirror Man"));
    assert!(!report.dms_to("Bob").contains("Copycat"));
    assert!(!report.dms_to("Bob").contains("Henchman"));
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Henchman #7");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Henchman #7");

    // Copycat viewing Mirror Man in Center #1 adopts Mirror Man passively and does NOT wake again at #-8-A:
    use crate::roles::vampire::copycat::COPYCAT;
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &HENCHMAN),
        ],
        [&MIRROR_MAN, &HENCHMAN, &INNOCENT_BYSTANDER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "1").await;
    let rep2 = h2.finish_night(n2).await;
    assert_eq!(rep2.effective_role_at(&h2.roster, "Alice"), "Mirror Man");
    assert!(rep2.dms_to("Alice").contains("no further actions tonight"));
}

#[tokio::test(start_paused = true)]
async fn mirror_man_center_doppelganger_dusk_assassin_and_order_independent_simultaneous_copiers() {
    use crate::marks::Mark;
    use crate::role::Team;
    use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
    use crate::roles::vampire::{
        assassin::{APPRENTICE_ASSASSIN, ASSASSIN},
        copycat::COPYCAT,
    };

    // (b) Mirror Man viewing center Doppelgänger in a Dusk game wakes at #-7, copies Assassin
    //     (OnRoleTurn #-1-A), stays hidden from Apprentice Assassin (#-1-B), and places Mark::Assassin!
    let mut h_dusk = OnuwTestHarness::new(
        &[
            ("Alice", &MIRROR_MAN),
            ("Bob", &ASSASSIN),
            ("Carol", &APPRENTICE_ASSASSIN),
            ("Dave", &VILLAGER),
        ],
        [&DOPPELGANGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_dusk = h_dusk.spawn_night();
    // #-8-A: Alice (Mirror Man) views Center #1 (Doppelgänger)
    h_dusk.reply_dm("Alice", "1").await;
    // #-7: Alice wakes as Mirror Man-Doppelgänger in Dusk and copies Bob (Assassin)
    h_dusk.reply_dm("Alice", "Bob").await;
    // #-1 & #-1-A: Bob (Assassin) marks Carol; Alice (Mirror Man-Assassin) marks Dave
    h_dusk.reply_dm("Bob", "Carol").await;
    h_dusk.reply_dm("Alice", "Dave").await;
    let rep_dusk = h_dusk.finish_night(n_dusk).await;

    assert!(rep_dusk.channel_lines.join("\n").contains("Dusk falls!"));
    assert_eq!(rep_dusk.board.mark(2), Mark::Assassin);
    assert_eq!(rep_dusk.board.mark(3), Mark::Assassin);
    // Apprentice Assassin (Carol) sees ONLY mentor Bob, NOT Mirror Man-Doppelgänger-Assassin Alice!
    assert!(rep_dusk
        .dms_to("Carol")
        .contains("Your mentor Assassin is Bob"));
    assert!(!rep_dusk.dms_to("Carol").contains("Alice"));
    assert!(rep_dusk.has_chronicle_substr(
        "Mirror Man-Doppelgänger (Alice) copied Bob and became Assassin."
    ));
    assert!(rep_dusk.has_chronicle_substr(
        "Mirror Man-Assassin (Alice) placed Mark of the Assassin on Dave."
    ));
    // Voting: Alice (Mirror Man-Doppelgänger-Assassin) and Dave (Mark::Assassin) tie at 2 votes and both die ->
    // Alice & Bob win (Mark::Assassin holder Dave died) AND Carol (Apprentice Assassin) wins (copied Assassin Alice died)!
    let dawn_dusk = h_dusk
        .resolve_voting(
            rep_dusk,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_dusk.winner_nicks(&h_dusk.roster),
        vec!["Alice", "Bob", "Carol"]
    );

    // (c1) Simultaneous [Copycat (Seat 0), Mirror Man (Seat 1)] both viewing center Doppelgänger:
    //      Seat 0 (Copycat Alice) copies Carol (Werewolf); Seat 1 (Mirror Man Bob) copies Seat 0 (Alice)!
    let mut h_ab = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &MIRROR_MAN),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&DOPPELGANGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_ab = h_ab.spawn_night();
    h_ab.reply_dm("Alice", "1").await;
    h_ab.reply_dm("Bob", "1").await;
    // At #1 (Doppelgänger), both Alice and Bob wake simultaneously:
    h_ab.wait_for_prompt_dm("Alice").await;
    h_ab.wait_for_prompt_dm("Bob").await;
    h_ab.send_dm("Alice", "Carol").await;
    h_ab.send_dm("Bob", "Alice").await;
    let rep_ab = h_ab.finish_night(n_ab).await;

    assert_eq!(rep_ab.effective_role_at(&h_ab.roster, "Alice"), "Werewolf");
    assert_eq!(rep_ab.board.effective_team(0), Team::Werewolf);
    assert_eq!(rep_ab.effective_role_at(&h_ab.roster, "Bob"), "Doppelgänger");
    assert_eq!(rep_ab.board.effective_team(1), Team::Village);
    assert!(rep_ab.dms_to("Alice").contains("Fellow Werewolf pack: Carol"));
    assert!(rep_ab.dms_to("Bob").contains("Alice's card is Copycat!"));
    assert!(!rep_ab.dms_to("Bob").contains("Werewolf"));
    assert!(!rep_ab.dms_to("Bob").contains("Fellow Werewolf pack"));
    assert!(rep_ab.has_chronicle_substr(
        "Mirror Man-Doppelgänger (Bob) copied Alice (Copycat → Doppelgänger)."
    ));

    // (c2) Reversed seat order [Mirror Man (Seat 0), Copycat (Seat 1)] with the EXACT same logical choices:
    //      Seat 0 (Mirror Man Bob) copies Seat 1 (Alice); Seat 1 (Copycat Alice) copies Carol (Werewolf)!
    let mut h_ba = OnuwTestHarness::new(
        &[
            ("Bob", &MIRROR_MAN),
            ("Alice", &COPYCAT),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&DOPPELGANGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_ba = h_ba.spawn_night();
    h_ba.reply_dm("Alice", "1").await;
    h_ba.reply_dm("Bob", "1").await;
    h_ba.wait_for_prompt_dm("Bob").await;
    h_ba.wait_for_prompt_dm("Alice").await;
    h_ba.send_dm("Bob", "Alice").await;
    h_ba.send_dm("Alice", "Carol").await;
    let rep_ba = h_ba.finish_night(n_ba).await;

    assert_eq!(rep_ba.effective_role_at(&h_ba.roster, "Bob"), "Doppelgänger");
    assert_eq!(rep_ba.board.effective_team(0), Team::Village);
    assert_eq!(rep_ba.effective_role_at(&h_ba.roster, "Alice"), "Werewolf");
    assert_eq!(rep_ba.board.effective_team(1), Team::Werewolf);
    assert!(rep_ba.dms_to("Alice").contains("Fellow Werewolf pack: Carol"));
    assert!(rep_ba.dms_to("Bob").contains("Alice's card is Copycat!"));
    assert!(!rep_ba.dms_to("Bob").contains("Werewolf"));
    assert!(!rep_ba.dms_to("Bob").contains("Fellow Werewolf pack"));
    assert!(rep_ba.has_chronicle_substr(
        "Mirror Man-Doppelgänger (Bob) copied Alice (Copycat → Doppelgänger)."
    ));
}

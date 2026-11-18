use crate::marks::Mark;
use crate::roles::base::{doppelganger::DOPPELGANGER, tanner::TANNER, villager::VILLAGER};
use crate::roles::daybreak::witch::WITCH;
use crate::roles::vampire::{
    assassin::{APPRENTICE_ASSASSIN, ASSASSIN},
    instigator::INSTIGATOR,
    vampire::VAMPIRE,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn assassin_and_apprentice_assassin_mutual_dusk_sight_bitten_mentor_and_center_swap_rules() {
    // 1. Mutual Dusk sight at #-1B:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ASSASSIN),
            ("Bob", &APPRENTICE_ASSASSIN),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    let rep = h.finish_night(n).await;
    assert!(rep.dms_to("Bob").contains("Your mentor Assassin is Alice"));
    assert!(rep
        .dms_to("Alice")
        .contains("Your Apprentice Assassin is Bob"));

    // 2. Apprentice Assassin wins when killing a Mark::Vampire-bitten Assassin:
    let mut h_aa = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &ASSASSIN),
            ("Carol", &APPRENTICE_ASSASSIN),
            ("Dave", &INSTIGATOR),
            ("Eve", &TANNER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_aa = h_aa.spawn_night();
    h_aa.reply_dm("Alice", "Bob").await;
    h_aa.reply_dm("Dave", "Alice").await;
    h_aa.reply_dm("Bob", "Dave").await;
    let rep_aa = h_aa.finish_night(n_aa).await;
    let dawn_aa = h_aa
        .resolve_voting(
            rep_aa,
            &[
                ("Alice", Some("Bob")),
                ("Carol", Some("Bob")),
                ("Bob", Some("Eve")),
                ("Dave", Some("Eve")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_aa.winner_nicks(&h_aa.roster),
        vec!["Alice", "Carol", "Dave", "Eve"]
    );

    // 3. Witch swapping Assassin into center prevents Apprentice Assassin from winning off Assassin's mark:
    let mut h_sw = OnuwTestHarness::new(
        &[
            ("Alice", &ASSASSIN),
            ("Bob", &APPRENTICE_ASSASSIN),
            ("Carol", &WITCH),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_sw = h_sw.spawn_night();
    h_sw.reply_dm("Alice", "Carol").await;
    h_sw.reply_dm("Carol", "1").await;
    h_sw.reply_dm("Carol", "Alice").await;
    let rep_sw = h_sw.finish_night(n_sw).await;
    let dawn_sw = h_sw
        .resolve_voting(
            rep_sw,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn_sw.winner_nicks(&h_sw.roster).is_empty());

    // 4. When no Mark of the Assassin is in play (Assassin / Apprentice Assassin started in center
    //    and Drunk swapped into them), they become simple Villagers and win with the Village!
    use crate::roles::base::drunk::DRUNK;
    let mut h_vil = OnuwTestHarness::new(
        &[("Alice", &DRUNK), ("Bob", &DRUNK), ("Carol", &VILLAGER)],
        [&ASSASSIN, &APPRENTICE_ASSASSIN, &VILLAGER],
    )
    .await;
    let n_vil = h_vil.spawn_night();
    h_vil.wait_for_prompt_dm("Alice").await;
    h_vil.wait_for_prompt_dm("Bob").await;
    h_vil.send_dm("Alice", "1").await;
    h_vil.send_dm("Bob", "2").await;
    let rep_vil = h_vil.finish_night(n_vil).await;
    let dawn_vil = h_vil
        .resolve_voting(
            rep_vil,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_vil.winner_nicks(&h_vil.roster),
        vec!["Alice", "Bob", "Carol"]
    );
}

#[tokio::test(start_paused = true)]
async fn doppelganger_assassin_is_hidden_from_apprentice_assassin_yet_satisfies_kill_victory() {
    // ONUV Rulebook: "The Apprentice Assassin doesn't know who the Doppelgänger-Assassin is,
    // but wins if either the Assassin or the Doppelgänger-Assassin is killed."
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &ASSASSIN),
            ("Carol", &APPRENTICE_ASSASSIN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let n = h.spawn_night();
    h.reply_dm("Alice", "Bob").await; // Dusk #-7: Alice becomes Doppelgänger-Assassin
    h.reply_dm("Bob", "Dave").await; // Dusk #-1: Assassin marks Dave
    h.reply_dm("Alice", "Dave").await; // Dusk #-1A: Doppelgänger-Assassin marks Dave
    let rep = h.finish_night(n).await;

    let carol_dms = rep.dms_to("Carol");
    assert!(
        carol_dms.contains("Your mentor Assassin is Bob"),
        "Carol DMs: {carol_dms}"
    );
    assert!(
        !carol_dms.contains("Alice"),
        "Apprentice Assassin must not learn Doppelgänger-Assassin Alice: {carol_dms}"
    );
    assert!(rep
        .dms_to("Bob")
        .contains("Your Apprentice Assassin is Carol"));
    assert!(
        !rep.dms_to("Alice").contains("Apprentice Assassin"),
        "Doppelgänger-Assassin must not see Apprentice Assassin"
    );

    // Executing Doppelgänger-Assassin Alice (while Bob & marked Dave survive) still wins the game for Carol!
    let dawn = h
        .resolve_voting(
            rep,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Carol"]);
}

#[tokio::test(start_paused = true)]
async fn doppelganger_apprentice_assassin_wakes_sequentially_without_leaks() {
    // 1. Assassin in center: Seat 1 Apprentice Assassin (Bob) wakes at #-1B BEFORE Seat 0
    //    Doppelgänger-Apprentice Assassin (Alice) at #-1Ba; each places their own Mark::Assassin
    //    with zero identity/target leaks, and both co-win if any Mark::Assassin holder dies:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &APPRENTICE_ASSASSIN),
            ("Carol", &VILLAGER),
            ("Dave", &VAMPIRE),
        ],
        [&ASSASSIN, &VILLAGER, &VILLAGER],
    )
    .await;

    let n = h.spawn_night();
    h.reply_dm("Alice", "Bob").await; // Dusk #-7: Alice copies Apprentice Assassin Bob
    h.reply_dm("Dave", "Carol").await; // Dusk #-6: Vampire Dave marks Carol
    h.reply_dm("Bob", "Carol").await; // Dusk #-1B (Slot 1/2): Bob marks Carol
    h.reply_dm("Alice", "Dave").await; // Dusk #-1Ba (Slot 2/2): Alice marks Dave
    let rep = h.finish_night(n).await;

    assert_eq!(rep.mark_at(&h.roster, "Carol"), Mark::Assassin);
    assert_eq!(rep.mark_at(&h.roster, "Dave"), Mark::Assassin);

    let bob_dms = rep.dms_to("Bob");
    let alice_dms = rep.dms_to("Alice");
    assert!(
        !bob_dms.contains("Fellow Apprentice")
            && !bob_dms.contains("Doppelgänger")
            && !bob_dms.contains("on Dave"),
        "Bob must not learn about Doppelgänger-Apprentice Assassin Alice or her target: {bob_dms}"
    );
    assert!(
        !alice_dms.contains("Fellow Apprentice") && !alice_dms.contains("on Carol"),
        "Alice must not learn Bob's target Carol: {alice_dms}"
    );

    // Executing Carol (holding Bob's Mark::Assassin) makes both Bob and Alice win alongside surviving Vampire Dave:
    let dawn = h
        .resolve_voting(
            rep,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Dave"]);

    // 2. Assassin in play: Assassin's eyes stay open through both #-1B and #-1Ba, while Bob (#-1B)
    //    never learns Alice woke at #-1Ba:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &APPRENTICE_ASSASSIN),
            ("Carol", &ASSASSIN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    h2.reply_dm("Carol", "Dave").await;
    let rep2 = h2.finish_night(n2).await;

    assert!(rep2.dms_to("Bob").contains("Your mentor Assassin is Carol"));
    assert!(!rep2.dms_to("Bob").contains("Alice"));
    assert!(rep2
        .dms_to("Alice")
        .contains("Your mentor Assassin is Carol"));
    let carol_mentor_dms = rep2.dms_to("Carol");
    assert!(carol_mentor_dms.contains("Your Apprentice Assassin is Bob"));
    assert!(carol_mentor_dms.contains("Your Doppelgänger-Apprentice Assassin is Alice"));
}

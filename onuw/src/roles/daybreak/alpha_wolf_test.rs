use ircgm::prelude::Text;

use crate::roles::base::{minion::MINION, seer::SEER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::{
    alpha_wolf::ALPHA_WOLF, dream_wolf::DREAM_WOLF, mystic_wolf::MYSTIC_WOLF, sentinel::SENTINEL,
};
use crate::roles::vampire::count::COUNT;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn alpha_wolf_and_mystic_wolf_wake_together_even_without_base_werewolf_in_deck() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &MYSTIC_WOLF),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Alpha Wolf) bites Carol; Bob (Mystic Wolf) inspects Carol; Carol (Seer) passes:
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Bob", "Carol").await;

    let report = h.finish_night(night).await;
    // Pack recognition happened automatically via companion_group_plugin(&WEREWOLF):
    assert!(report.dms_to("Alice").contains("Fellow Werewolf pack: Bob"));
    assert!(report.dms_to("Bob").contains("Fellow Werewolf pack: Alice"));
    // Alpha Wolf (122) bit Carol BEFORE Mystic Wolf (123) resolved, so Carol became Werewolf!
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert!(report.dms_to("Bob").contains("Carol's card is: Werewolf"));
    assert!(report.has_chronicle_substr("Alpha Wolf (Alice) bit Carol, swapping Werewolf in for Seer."));
}

#[tokio::test(start_paused = true)]
async fn alpha_wolf_prompt_excludes_waking_wolves_and_dream_wolf_keeps_shields_and_hides_feared_wolf() {
    // 1. Standard Daybreak: Alpha Wolf (Alice), Mystic Wolf (Bob), Dream Wolf (Carol),
    //    Sentinel (Dave -> shields Eve), Minion (Eve), Villager (Frank).
    //    Non-Wolves shown to Alice must exclude Alice, Bob, and Carol, while keeping Minion (Eve🛡) and shields!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &MYSTIC_WOLF),
            ("Carol", &DREAM_WOLF),
            ("Dave", &SENTINEL),
            ("Eve", &MINION),
            ("Frank", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Sentinel (Dave) shields Eve (Minion):
    h.reply_dm("Dave", "Eve").await;
    // Alpha Wolf (Alice) and Mystic Wolf (Bob):
    h.reply_dm("Alice", "Frank").await;
    h.reply_dm("Bob", "pass").await;

    let report = h.finish_night(night).await;
    let alice_plain = Text::parse(&report.dms_to("Alice")).plain();
    assert!(
        alice_plain.contains("On the table: Eve🛡")
            && alice_plain.contains("Non-Wolves: Dave, Eve, Frank"),
        "unexpected Alpha Wolf DMs:\n{alice_plain}"
    );

    // 2. Vampire crossover: The Count (Dave) paralyzes Dream Wolf (Carol) with Mark::Fear in Dusk.
    //    Carol cannot put out her thumb at Night, so Alpha Wolf (Alice) never learns Carol is a Wolf —
    //    Carol must remain in `Non-Wolves:` (no hidden-info leak!) and be a valid swap target!
    let mut h_fear = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &WEREWOLF),
            ("Carol", &DREAM_WOLF),
            ("Dave", &COUNT),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let n_fear = h_fear.spawn_night();
    // Dusk: Count (Dave) marks Eve with Mark::Vampire in Coven, then places Mark::Fear on Carol (Dream Wolf):
    h_fear.reply_dm("Dave", "Eve").await;
    h_fear.reply_dm("Dave", "Carol").await;
    // Night: Alice (Alpha Wolf) sees only Bob in pack wake, sees Carol in Non-Wolves, and bites Carol!
    h_fear.reply_dm("Alice", "Carol").await;

    let rep_fear = h_fear.finish_night(n_fear).await;
    let dave_plain = Text::parse(&rep_fear.dms_to("Dave")).plain();
    assert!(
        dave_plain.contains("Non-Vampires: Alice, Bob, Carol, Eve"),
        "unexpected Coven DMs:\n{dave_plain}"
    );
    assert!(
        dave_plain.contains("Non-Vampires: Alice, Bob, Carol\n")
            || dave_plain.ends_with("Non-Vampires: Alice, Bob, Carol"),
        "Count prompt should exclude both Dave and newly-turned Eve:\n{dave_plain}"
    );

    let alice_fear_plain = Text::parse(&rep_fear.dms_to("Alice")).plain();
    assert!(!alice_fear_plain.contains("Dream Wolf"));
    assert!(
        alice_fear_plain.contains("Non-Wolves: Carol, Dave, Eve"),
        "feared Dream Wolf must stay in Non-Wolves so Mark::Fear does not leak role:\n{alice_fear_plain}"
    );
    assert_eq!(rep_fear.effective_role_at(&h_fear.roster, "Carol"), "Werewolf");
}

#[tokio::test(start_paused = true)]
async fn extra_center_slots_support_wolf_and_villain_and_reject_numbers_or_ambiguous_extra() {
    use crate::roles::base::drunk::DRUNK;
    use crate::roles::daybreak::{apprentice_seer::APPRENTICE_SEER, witch::WITCH};
    use crate::roles::super_villains::{rapscallion::RAPSCALLION, temptress::TEMPTRESS};

    // 1. Alpha Wolf (Bob) bites Alice (Seer) at #2-B, moving Seer into Center Wolf:
    //    - Number "4" is rejected (perpendicular card is not a 4th numbered slot!).
    //    - Apprentice Seer (Carol, #5-B) peeks "wolf" and sees Seer!
    //    - Witch (Dave, #6-B) peeks "perp" (unambiguous single extra card), sees Seer mid-prompt,
    //      and swaps Seer onto himself (moving Witch into Center Wolf)!
    //    - Drunk (Eve, #8) swaps with "cw", taking Witch and leaving Drunk in Center Wolf!
    //    - `!reveal` displays `Center Wolf: Drunk`!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SEER),
            ("Bob", &ALPHA_WOLF),
            ("Carol", &APPRENTICE_SEER),
            ("Dave", &WITCH),
            ("Eve", &DRUNK),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    assert!(h.roster.parse_center_slot("4").is_none());

    let night = h.spawn_night();
    h.reply_dm("Bob", "pass").await; // Lone Wolf peek (#2)
    h.reply_dm("Bob", "Alice").await; // Alpha Wolf (#2-B) bites Alice -> Seer moves to Center Wolf!
    h.reply_dm("Alice", "pass").await; // Seer (#5) passes
    h.wait_for_prompt_dm("Carol").await;
    h.send_dm("Carol", "4").await;
    let rej_four_aw = h.wait_for_new_dm("Carol").await;
    assert!(
        rej_four_aw.contains("Reply 1, 2, 3, wolf, or pass"),
        "got: {rej_four_aw}"
    );
    h.send_dm("Carol", "wolf").await; // Apprentice Seer (#5-B) peeks Center Wolf
    // Witch (#6-B) runs after Alpha Wolf, peeks "perp", sees Seer, and swaps onto Dave (self):
    h.reply_dm("Dave", "perp").await;
    h.reply_dm("Dave", "Dave").await;
    // Drunk (#8) swaps blindly with "cw" (Center Wolf):
    h.reply_dm("Eve", "cw").await;

    let report = h.finish_night(night).await;
    assert!(
        report.dms_to("Carol").contains("Center Wolf is: Seer"),
        "Apprentice Seer DMs:\n{}",
        report.dms_to("Carol")
    );
    assert!(
        report.dms_to("Dave").contains("Center Wolf is Seer!"),
        "Witch DMs:\n{}",
        report.dms_to("Dave")
    );
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Seer");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Witch");
    assert_eq!(report.extra_role("alpha_wolf"), "Drunk");

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Bob")),
            ],
        )
        .await;
    assert!(
        dawn.reveal_text().contains("Center Wolf: Drunk"),
        "expected Center Wolf in !reveal, got:\n{}",
        dawn.reveal_text()
    );

    // 2. Temptress game (without Alpha Wolf): "4" and "wolf" are rejected, while "villain" / "extra"
    //    let center roles view Carol's displaced Apprentice Seer card in `Extra Villain`!
    let mut h_sv = OnuwTestHarness::new(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &RAPSCALLION),
            ("Carol", &APPRENTICE_SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    assert!(h_sv.roster.parse_center_slot("4").is_none());
    assert!(h_sv.roster.parse_center_slot("wolf").is_none());

    let n_sv = h_sv.spawn_night();
    h_sv.reply_dm("Alice", "Carol").await; // Temptress (#1-SV-B) swaps Extra Villain onto Carol!
    h_sv.wait_for_prompt_dm("Bob").await;
    h_sv.send_dm("Bob", "wolf").await;
    let rej_wolf = h_sv.wait_for_new_dm("Bob").await;
    assert!(
        rej_wolf.contains("Reply 1, 2, 3, villain, or pass"),
        "got: {rej_wolf}"
    );
    h_sv.send_dm("Bob", "villain").await; // Rapscallion (#1-SV-D) peeks Extra Villain
    h_sv.reply_dm("Carol", "extra").await; // Apprentice Seer (#5-B) peeks Extra Villain via "extra"

    let rep_sv = h_sv.finish_night(n_sv).await;
    assert!(
        rep_sv
            .dms_to("Bob")
            .contains("Extra Villain is Apprentice Seer!"),
        "Rapscallion DMs:\n{}",
        rep_sv.dms_to("Bob")
    );
    assert!(
        rep_sv
            .dms_to("Carol")
            .contains("Extra Villain is: Apprentice Seer"),
        "Apprentice Seer DMs:\n{}",
        rep_sv.dms_to("Carol")
    );
    let dawn_sv = h_sv
        .resolve_voting(
            rep_sv,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert!(
        dawn_sv
            .reveal_text()
            .contains("Extra Villain: Apprentice Seer"),
        "expected Extra Villain in !reveal, got:\n{}",
        dawn_sv.reveal_text()
    );

    // 3. Alpha Wolf + Temptress crossover: "extra" / "perp" is rejected as ambiguous, while Seer
    //    can run `center wolf villain` to inspect BOTH perpendicular cards at once!
    let mut h_both = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &TEMPTRESS),
            ("Carol", &SEER),
            ("Dave", &DRUNK),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    assert!(h_both.roster.parse_center_slot("extra").is_none());
    assert!(h_both.roster.parse_center_slot("perp").is_none());

    let n_both = h_both.spawn_night();
    h_both.reply_dm("Bob", "Dave").await; // Temptress (#1-SV-B) swaps Henchman #7 onto Dave -> Drunk in Extra Villain
    h_both.reply_dm("Alice", "pass").await; // Lone Wolf peek (#2)
    h_both.reply_dm("Alice", "Eve").await; // Alpha Wolf (#2-B) bites Eve -> Villager in Center Wolf
    h_both.wait_for_prompt_dm("Carol").await;
    h_both.send_dm("Carol", "center 1 extra").await;
    let rej_ambig = h_both.wait_for_new_dm("Carol").await;
    assert!(
        rej_ambig.contains("Center slots must be 1, 2, 3, wolf, or villain."),
        "got: {rej_ambig}"
    );
    h_both.send_dm("Carol", "center wolf villain").await;
    h_both.reply_dm("Dave", "villain").await;

    let rep_both = h_both.finish_night(n_both).await;
    assert!(
        rep_both
            .dms_to("Carol")
            .contains("Center Wolf is Villager, Extra Villain is Drunk."),
        "Seer DMs:\n{}",
        rep_both.dms_to("Carol")
    );
}

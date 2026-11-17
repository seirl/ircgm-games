use crate::marks::Mark;
use crate::roles::base::{
    doppelganger::DOPPELGANGER, hunter::HUNTER, robber::ROBBER, seer::SEER, villager::VILLAGER,
};
use crate::roles::vampire::{
    assassin::{APPRENTICE_ASSASSIN, ASSASSIN},
    copycat::COPYCAT,
    count::COUNT,
    cupid::CUPID,
    diseased::DISEASED,
    instigator::INSTIGATOR,
    marksman::MARKSMAN,
    pickpocket::PICKPOCKET,
    priest::PRIEST,
    renfield::RENFIELD,
    the_master::THE_MASTER,
    vampire::VAMPIRE,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn vampire_coven_rejects_talking_relays_silent_points_and_count_fears_seer() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &COUNT),
            ("Carol", &RENFIELD),
            ("Dave", &SEER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Wait for both Alice and Bob to receive their Vampire Coven silent-pointing prompts:
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Bob").await;

    // Talking at night is rejected privately and never relayed to Bob:
    h.send_dm("Alice", "let's turn Eve and fear Dave?").await;
    let alice_rej = h.wait_for_new_dm("Alice").await;
    assert!(alice_rej.contains("Silent night"), "got: {alice_rej}");

    // Alice points at Dave -> relayed as a silent point to Bob; then Bob points at Eve (latest pointer wins 1-1 tie, or Alice also points Eve!):
    h.send_dm("Alice", "Dave").await;
    h.send_dm("Bob", "Eve").await;
    // In Dusk (#-6B, The Count), Bob fears Dave (Seer), while Carol (Renfield, #-6C) places Mark of the Bat on herself:
    h.reply_dm("Bob", "Dave").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    assert!(!bob_dms.contains("let's turn Eve"));
    assert!(bob_dms.contains("Alice points at Dave!"));
    assert!(report.dms_to("Alice").contains("Bob points at Eve!"));
    assert_eq!(report.mark_at(&h.roster, "Eve"), Mark::Vampire);
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Bat);
    assert_eq!(report.mark_at(&h.roster, "Dave"), Mark::Fear);
    assert!(report.has_chronicle_substr("Vampire Coven (Alice, Bob) bestowed Mark of the Vampire on Eve."));
    assert!(report.has_chronicle_substr("The Count (Bob) placed Mark of Fear on Dave."));
}

#[tokio::test(start_paused = true)]
async fn count_mark_of_fear_suppresses_night_seer_completely() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &COUNT),
            ("Bob", &SEER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Solo Count in Coven marks Carol with Vampire, then marks Bob (Seer) with Fear:
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Fear);
    assert!(report
        .dms_to("Bob")
        .contains("too terrified to wake up tonight"));
    assert!(!report.dms_to("Bob").contains("🔮 Seer:"));
    assert!(report.has_chronicle_substr(
        "Bob was paralyzed by the Mark of Fear and could not wake up during Night!"
    ));
}

#[tokio::test(start_paused = true)]
async fn copycat_runs_before_doppelganger_and_doppel_copycat_follows_cardboard_rule() {
    use crate::roles::base::tanner::TANNER;
    use crate::roles::base::werewolf::WEREWOLF;
    use crate::roles::daybreak::paranormal_investigator::PARANORMAL_INVESTIGATOR;
    use crate::roles::daybreak::witch::WITCH;

    // 1. Copycat (#-8) peeks Werewolf in Center #1; Doppelgänger copies Copycat, sees ONLY "Copycat",
    //    stays asleep (does NOT wake in Werewolf pack), and inherits Werewolf for endgame!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &COPYCAT),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Bob", "1").await; // Copycat -> Werewolf
    h.reply_dm("Alice", "Bob").await; // Doppelgänger -> Copycat (sees "Copycat", stays asleep!)
    // Night #2: ONLY Bob wakes as Lone Wolf (Alice does NOT wake because she doesn't know Bob saw Werewolf!):
    h.reply_dm("Bob", "pass").await;

    let report = h.finish_night(night).await;
    assert!(!report.channel_lines.join("\n").contains("Dusk falls!"));
    assert!(report.dms_to("Alice").contains("Bob's card is"));
    assert!(report.dms_to("Alice").contains("Copycat"));
    assert!(!report.dms_to("Alice").contains("Werewolf"));
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");

    // 2. Edge case `2.b`: Copycat peeks Paranormal Investigator in center, Doppelgänger copies Copycat,
    //    then Copycat-PI peeks Tanner at #5C and becomes Tanner — Doppel-Copycat stays a Village PI!
    let mut h_pi = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &COPYCAT),
            ("Carol", &TANNER),
        ],
        [&PARANORMAL_INVESTIGATOR, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_pi = h_pi.spawn_night();
    h_pi.reply_dm("Bob", "1").await;
    h_pi.reply_dm("Alice", "Bob").await;
    h_pi.reply_dm("Bob", "Carol").await; // Copycat-PI inspects Tanner Carol and transforms!
    let rep_pi = h_pi.finish_night(n_pi).await;
    assert_eq!(rep_pi.effective_role_at(&h_pi.roster, "Bob"), "Tanner");
    assert_eq!(
        rep_pi.effective_role_at(&h_pi.roster, "Alice"),
        "Paranormal Investigator"
    );

    // 3. Apprentice Assassin wins when killing a Mark::Vampire-bitten Assassin, and does NOT win
    //    off Mark::Assassin if Witch swapped the Assassin card into the center; also Monster Traitor
    //    wins alongside Tanner when fellow Monster + Tanner both die!
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
    h_aa.reply_dm("Alice", "Bob").await; // Vampire bites Assassin Bob!
    h_aa.reply_dm("Dave", "Alice").await; // Instigator gives Vampire Alice Mark::Traitor!
    h_aa.reply_dm("Bob", "Dave").await; // Assassin Bob marks Dave
    let rep_aa = h_aa.finish_night(n_aa).await;
    // Execute Bob (Vampire-bitten Assassin, 2 votes) AND Eve (Tanner, 2 votes):
    // - Carol (Apprentice Assassin) wins because Assassin Bob died (even with Mark::Vampire)!
    // - Eve (Tanner) wins!
    // - Village (Dave) wins because primary Vampire threat Bob died!
    // - Alice (Vampire + Mark::Traitor) ALSO wins because fellow Vampire Bob died and Tanner only blocks the Vampire team, not a personal Traitor!
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

    // And if Witch swaps Assassin into the center during Night, Apprentice Assassin (who saw Assassin
    // at Dusk and placed no solo mark) does NOT win off Assassin's Mark::Assassin:
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
    h_sw.reply_dm("Alice", "Carol").await; // Assassin marks Carol
    h_sw.reply_dm("Carol", "1").await; // Witch peeks Center #1 (Villager)
    h_sw.reply_dm("Carol", "Alice").await; // Witch swaps Villager onto Alice (moving Assassin into Center #1!)
    let rep_sw = h_sw.finish_night(n_sw).await;
    // Execute Carol (who holds Mark::Assassin, while Assassin is now in Center #1):
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
}

#[tokio::test(start_paused = true)]
async fn priest_pickpocket_marksman_instigator_cupid_diseased_master_and_assassins_work() {
    // 1. Priest cleanses Mark::Vampire & Pickpocket steals Mark::Assassin & Marksman scopes both:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ASSASSIN),
            ("Bob", &PRIEST),
            ("Carol", &PICKPOCKET),
            ("Dave", &MARKSMAN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Dusk: Priest (#-2) passes extra cleanse, Assassin (#-1) marks Dave!
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Alice", "Dave").await;
    // Night: Marksman (Dave, #5D) scopes Alice's card and Bob's mark BEFORE Pickpocket (Carol, #6C) steals Mark::Assassin from Dave!
    h.reply_dm("Dave", "Alice Bob").await;
    h.reply_dm("Carol", "Dave").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Assassin);
    assert_eq!(report.mark_at(&h.roster, "Dave"), Mark::Clarity);
    assert!(report
        .dms_to("Dave")
        .contains("Alice's card is Assassin, and Bob's mark is Mark of Clarity!"));
    assert!(report.has_chronicle_substr(
        "Pickpocket (Carol) stole Mark of the Assassin from Dave (giving Dave Mark of Clarity)."
    ));

    // Kill Carol (who now holds Mark of the Assassin) -> Assassin Alice wins!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice"]);

    // 2. Cupid heartbreak chains into Hunter + The Master vote immunity + Instigator Traitor + Disease penalty:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &THE_MASTER),
            ("Bob", &VAMPIRE),
            ("Carol", &CUPID),
            ("Dave", &HUNTER),
            ("Eve", &DISEASED),
            ("Frank", &INSTIGATOR),
            ("Grace", &APPRENTICE_ASSASSIN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night2 = h2.spawn_night();
    h2.wait_for_prompt_dm("Alice").await;
    h2.wait_for_prompt_dm("Bob").await;
    h2.send_dm("Bob", "Grace").await;
    // Eve (Diseased, #-5) places Mark::Disease on her immediate right neighbor Frank:
    h2.reply_dm("Eve", "Frank").await;
    // Carol (Cupid, #-4) links Carol and Dave with Mark::Love:
    h2.reply_dm("Carol", "Carol Dave").await;
    // Frank (Instigator, #-3) places Mark::Traitor on Alice (The Master — wins if fellow Vampire dies while surviving!):
    h2.reply_dm("Frank", "Alice").await;
    // Grace (Apprentice Assassin, #-1B) places Mark::Assassin on Eve:
    h2.reply_dm("Grace", "Eve").await;

    let report2 = h2.finish_night(night2).await;
    // At Dawn:
    // - Alice (The Master) gets 2 votes (Carol, Frank), BUT Bob (Vampire) points at Alice -> Alice is immune!
    // - Carol gets 2 votes (from Alice and Grace) -> Carol dies!
    // - Carol & Dave have Mark of Love -> Dave (Hunter) dies of heartbreak!
    // - Dave (Hunter) pointed at Bob (Vampire) -> Hunter shoots Bob!
    // - A Vampire (Bob) died -> Village team (Carol, Dave, Eve, Frank) wins, EXCEPT Eve voted for Frank (`Mark::Disease`)
    //   so Eve forfeits her win! Meanwhile Alice (`Team::Vampire` + `Mark::Traitor`) survived while fellow Vampire Bob died
    //   -> Alice ALSO wins as Traitor!
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Frank")),
                ("Frank", Some("Alice")),
                ("Grace", Some("Carol")),
            ],
        )
        .await;
    let dead = dawn2.dead_nicks(&h2.roster);
    assert!(!dead.contains("Alice")); // The Master survived via Bob's Vampire vote!
    assert!(dead.contains("Carol"));
    assert!(dead.contains("Dave")); // Lover heartbreak!
    assert!(dead.contains("Bob")); // Hunter shot!
    assert_eq!(
        dawn2.winner_nicks(&h2.roster),
        vec!["Alice", "Carol", "Dave", "Frank"]
    );
}

#[tokio::test(start_paused = true)]
async fn doppelganger_marksman_wakes_in_night_after_dusk_marks_are_placed() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MARKSMAN),
            ("Carol", &VAMPIRE),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Dusk #-7: Alice copies Bob (Marksman) — she does NOT inspect Marks yet because Dusk Marks haven't been placed!
    h.reply_dm("Alice", "Bob").await;
    // Dusk #-6: Carol (Vampire) bites Dave with Mark of the Vampire:
    h.reply_dm("Carol", "Dave").await;
    // Night #5-B: Bob (Marksman) passes while Alice (Doppelgänger-Marksman) inspects Carol's card and Dave's Mark:
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Alice", "Carol Dave").await;

    let report = h.finish_night(night).await;
    assert!(
        report
            .dms_to("Alice")
            .contains("Dave's mark is Mark of the Vampire"),
        "Alice DMs: {}",
        report.dms_to("Alice")
    );
}

#[tokio::test(start_paused = true)]
async fn assassin_mutual_sight_copycat_doppelganger_sleep_and_doppelganger_renfield_thrall_work() {
    // 1. Assassin & Apprentice Assassin both see each other at Dusk #-1B:
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

    // 2. Copycat viewing Doppelgänger in center becomes Village Doppelgänger and does NOT wake again:
    let mut h_cc = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &ROBBER),
            ("Carol", &VILLAGER),
        ],
        [&DOPPELGANGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_cc = h_cc.spawn_night();
    h_cc.reply_dm("Alice", "1").await;
    h_cc.reply_dm("Bob", "pass").await;
    let rep_cc = h_cc.finish_night(n_cc).await;
    assert_eq!(rep_cc.effective_role_at(&h_cc.roster, "Alice"), "Doppelgänger");
    assert!(!rep_cc.dms_to("Alice").contains("🎭 Doppelgänger:"));
    assert!(rep_cc
        .dms_to("Alice")
        .contains("You have no further actions tonight."));

    // 3. Doppelgänger-Renfield still sees Mark::Vampire even when Vampires marked the initial Renfield:
    let mut h_ren = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &RENFIELD),
            ("Carol", &VAMPIRE),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_ren = h_ren.spawn_night();
    h_ren.reply_dm("Alice", "Bob").await; // Doppelgänger -> Renfield
    h_ren.reply_dm("Carol", "Bob").await; // Vampire marks Renfield Bob!
    let rep_ren = h_ren.finish_night(n_ren).await;
    assert_eq!(rep_ren.mark_at(&h_ren.roster, "Bob"), Mark::Bat);
    assert_eq!(rep_ren.mark_at(&h_ren.roster, "Alice"), Mark::Bat);
    assert!(rep_ren
        .dms_to("Alice")
        .contains("Marked with \u{2}\u{3}04Mark of the Vampire\u{f}: Bob")
        || rep_ren.dms_to("Alice").contains("Mark of the Vampire")
            && rep_ren.dms_to("Alice").contains(": Bob"));
}

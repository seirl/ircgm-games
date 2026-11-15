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
async fn vampire_coven_huddle_relays_whispers_marks_thrall_and_count_fears_seer() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &COUNT),
            ("Carol", &RENFIELD),
            ("Dave", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Wait for both Alice and Bob to enter the Vampire Coven Huddle:
    let _ = h.wait_for_new_dm("Alice").await;
    let _ = h.wait_for_new_dm("Bob").await;

    // Alice whispers to Bob in the Coven Huddle -> relayed to Bob!
    h.send_dm("Alice", "let's turn Dave or Carol?").await;
    // Bob locks in `!mark Dave` in Dusk Tier 1, then in Dusk Tier 2 (`Count`) fears Carol:
    h.send_dm("Bob", "!mark Dave").await;
    h.reply_dm("Bob", "Carol").await;
    // Stage B (Night): Dave (Seer, not feared since Carol got Fear) peeks Alice:
    h.reply_dm("Dave", "Alice").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Bob")
        .contains("[Coven] <Alice> let's turn Dave or Carol?"));
    assert_eq!(report.mark_at(&h.roster, "Dave"), Mark::Vampire);
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Fear);
    // Dave is Seer (NOT terrified since Carol got Fear) — wait: Dave wasn't answered above?
    // Oh! Dave=Seer DID wake in Stage B and timed out? No, `start_paused = true` auto-advances when all tasks sleep!
    assert!(report.has_chronicle_substr("Vampire Coven (Alice, Bob) bestowed Mark of the Vampire on Dave."));
    assert!(report.has_chronicle_substr("Count (Bob) placed Mark of Fear on Carol."));
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
async fn copycat_runs_before_doppelganger_so_chained_copy_propagates_center_role() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &COPYCAT),
            ("Carol", &VILLAGER),
        ],
        [&ROBBER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Copycat (wake_order 5) runs BEFORE Doppelgänger (wake_order 10) even though Alice is seat #1!
    h.reply_dm("Bob", "1").await;
    h.reply_dm("Bob", "pass").await;
    // Alice (Doppelgänger) copies Bob (Copycat -> Robber!) and steals Carol's Villager card:
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Robber");
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Villager");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Robber");
    assert!(report.has_chronicle_substr("Copycat (Bob) copied Center #1 and became Robber."));
    assert!(report.has_chronicle_substr("Doppelgänger (Alice) copied Bob and became Robber."));
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
    // Dusk: Assassin marks Dave, Priest passes extra cleanse, Pickpocket swaps Marks with Dave!
    h.reply_dm("Alice", "Dave").await;
    h.reply_dm("Bob", "pass").await;
    h.reply_dm("Carol", "Dave").await;
    // Night: Marksman (Dave) inspects Alice's card and Carol's mark!
    h.reply_dm("Dave", "Alice Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Assassin);
    assert_eq!(report.mark_at(&h.roster, "Dave"), Mark::Clarity);
    assert!(report
        .dms_to("Dave")
        .contains("Alice's card is Assassin, and Carol's mark is Mark of the Assassin!"));
    assert!(report.has_chronicle_substr(
        "Pickpocket (Carol) stole Mark of the Assassin from Dave (giving Dave Mark of Clarity)."
    ));

    // Kill Carol (who now holds Mark of the Assassin) -> Assassin Alice wins!
    let dawn = h
        .resolve_dawn(
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
    let _ = h2.wait_for_new_dm("Alice").await;
    let _ = h2.wait_for_new_dm("Bob").await;
    h2.send_dm("Bob", "!mark Grace").await;
    h2.reply_dm("Carol", "Carol Dave").await;
    h2.reply_dm("Eve", "Frank").await;
    h2.reply_dm("Frank", "Frank").await;
    h2.reply_dm("Grace", "Alice").await;

    let report2 = h2.finish_night(night2).await;
    // At Dawn:
    // - Alice (The Master) gets 3 votes, BUT Bob (Vampire) points at Alice -> Alice is immune!
    // - Carol gets 2 votes (from Alice and Frank) -> Carol dies!
    // - Carol & Dave have Mark of Love -> Dave (Hunter) dies of heartbreak!
    // - Dave (Hunter) pointed at Bob (Vampire) -> Hunter shoots Bob!
    // - A Vampire (Bob) died -> Village team wins, EXCEPT Frank has Mark::Traitor (turned Evil)
    //   and anyone who voted for Frank (Mark::Disease) loses their win!
    let dawn2 = h2
        .resolve_dawn(
            report2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Alice")),
                ("Frank", Some("Carol")),
                ("Grace", Some("Frank")),
            ],
        )
        .await;
    let dead = dawn2.dead_nicks(&h2.roster);
    assert!(!dead.contains("Alice")); // The Master survived via Bob's Vampire vote!
    assert!(dead.contains("Carol"));
    assert!(dead.contains("Dave")); // Lover heartbreak!
    assert!(dead.contains("Bob")); // Hunter shot!
    // Village wins (Carol, Dave, Eve), while Frank (Mark::Traitor -> Evil) loses!
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol", "Dave", "Eve"]);
}

use crate::roles::base::{
    robber::ROBBER, seer::SEER, troublemaker::TROUBLEMAKER, villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::roles::super_villains::{
    annoying_lad::ANNOYING_LAD,
    dr_peeker::DR_PEEKER,
    evilometer::EVILOMETER,
    henchman::HENCHMAN,
    mad_scientist_intern::{INTERN, MAD_SCIENTIST},
    rapscallion::RAPSCALLION,
    role_retriever::ROLE_RETRIEVER,
    temptress::TEMPTRESS,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn role_retriever_runs_after_robber_and_dawn_withholds_chronicle_from_channel() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ROBBER),
            ("Bob", &ROLE_RETRIEVER),
            ("Carol", &TROUBLEMAKER),
            ("Dave", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Dave (Lone Wolf) passes:
    h.reply_dm("Dave", "pass").await;
    // Alice (Robber, 160) steals Dave's Werewolf -> Alice=Werewolf, Dave=Robber:
    h.reply_dm("Alice", "Dave").await;
    // Bob (Role Retriever, 162) fetches Alice's new Werewolf -> Bob=Werewolf, Alice=Role Retriever:
    h.reply_dm("Bob", "Alice").await;
    // Carol (Troublemaker, 170) swaps Bob (now Werewolf) and Dave (now Robber) -> Bob=Robber, Dave=Werewolf:
    h.reply_dm("Carol", "Bob Dave").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Role Retriever");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Robber");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Werewolf");
    // Swapped target Alice must NOT be notified by Role Retriever:
    assert!(!report.dms_to("Alice").contains("Role Retriever swapped"));

    // Verify Dawn channel output withholds the chronicle while `reveal_text()` holds the full `📜 What Happened:` log:
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    let chan = dawn.channel_text();
    assert!(!chan.contains("📜 What Happened:"));
    let reveal = dawn.reveal_text();
    assert!(reveal.contains("📜 What Happened:"));
    assert!(reveal.contains("Robber (Alice) stole Werewolf from Dave (giving Dave Robber)."));
    assert!(reveal.contains(
        "Role Retriever (Bob) fetched Werewolf from Alice (giving Alice Role Retriever)."
    ));
    assert!(reveal.contains("Troublemaker (Carol) swapped Bob (Werewolf) and Dave (Robber)."));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Carol"]);
}

#[tokio::test(start_paused = true)]
async fn temptress_dr_peeker_evilometer_role_retriever_and_annoying_lad_work() {
    let mut h = OnuwTestHarness::with_extra_evil(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &ROLE_RETRIEVER),
            ("Carol", &DR_PEEKER),
            ("Dave", &EVILOMETER),
            ("Eve", &ANNOYING_LAD),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
        &HENCHMAN,
    )
    .await;

    let night = h.spawn_night();
    // Wait for Alice and Carol to finish the Super Villain Syndicate huddle, then act in the Villain solo tier:
    // Dr. Peeker (Carol, 126) X-rays fellow Villain Alice BEFORE Temptress (Alice, 128) plants Henchman #7 onto Eve!
    h.reply_dm("Carol", "Alice").await;
    h.reply_dm("Alice", "Eve").await;
    // Annoying Lad (Eve, 142 — #4C) pokes left neighbor Dave (poking Dave's right shoulder!):
    h.reply_dm("Eve", "left").await;
    // Role Retriever (Bob, 162 — #6A) swaps with Eve (who now holds Henchman #7!):
    h.reply_dm("Bob", "Eve").await;

    let report = h.finish_night(night).await;
    // Temptress prompt excludes fellow waking Villains (Alice, Carol):
    assert!(report.dms_to("Alice").contains("Non-Villains: Bob, Dave, Eve"));
    // Villains spot Evilometer (Dave), and Evilometer (Dave) beeps for adjacent Carol (Dr. Peeker):
    assert!(report.dms_to("Alice").contains("Dave holds the Evilometer"));
    assert!(report.dms_to("Carol").contains("Dave holds the Evilometer"));
    assert!(report.dms_to("Carol").contains("Alice holds Temptress"));
    assert!(report.dms_to("Dave").contains("*BEEP! BEEP!*"));
    assert!(report.dms_to("Dave").contains("poked your right shoulder"));
    // Role Retriever (Bob) sees his new card (Henchman #7), while Eve is NOT notified:
    assert!(report.dms_to("Bob").contains("you fetched Henchman #7!"));
    assert!(!report.dms_to("Eve").contains("Role Retriever"));
}

#[tokio::test(start_paused = true)]
async fn rapscallion_center_peek_and_mad_scientist_intern_co_win_blocking_all_teams() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &RAPSCALLION),
            ("Bob", &MAD_SCIENTIST),
            ("Carol", &INTERN),
            ("Dave", &VILLAGER),
        ],
        [&SEER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Rapscallion (Alice) wakes solo (no extra Solo Villain prompt!) and peeks Center #1 (Seer):
    h.reply_dm("Alice", "1").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Center #1 is Seer"));
    assert!(report.dms_to("Carol").contains("The Mad Scientist is Bob"));

    // Even when BOTH Alice (Rapscallion, Villain) AND Bob (Mad Scientist) die on a 2-2 tie,
    // Mad Scientist & Intern block ALL teams (including Village!) so ONLY Bob and Carol win!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);
}

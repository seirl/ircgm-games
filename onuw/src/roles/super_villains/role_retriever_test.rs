use crate::roles::base::{
    robber::ROBBER, troublemaker::TROUBLEMAKER, villager::VILLAGER, werewolf::WEREWOLF,
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
async fn role_retriever_notifies_victim_and_evilometer_detects_villain_at_villain_wake() {
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
    // Evilometer (Dave, 125) runs during the #2 Villain phase before swaps and detects adjacent Carol (Dr. Peeker)!
    // Temptress (Alice, 126) swaps her Temptress card with Eve (Annoying Lad):
    h.reply_dm("Alice", "Eve").await;
    // Dr. Peeker (Carol, 128) X-rays Bob (Role Retriever):
    h.reply_dm("Carol", "Bob").await;
    // Annoying Lad (Eve, 142 — #4C) pokes left neighbor Dave:
    h.reply_dm("Eve", "left").await;
    // Role Retriever (Bob, 162 — #6A) swaps with Alice (who now holds Annoying Lad!):
    h.reply_dm("Bob", "Alice").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Alice")
        .contains("The Role Retriever swapped your card! Your card is now Role Retriever!"));
    assert!(report
        .dms_to("Bob")
        .contains("you fetched Annoying Lad!"));
    assert!(report.dms_to("Dave").contains("*BEEP! BEEP!*"));
    assert!(report.dms_to("Dave").contains("*POKE!*"));
}

#[tokio::test(start_paused = true)]
async fn rapscallion_mad_scientist_and_intern_co_win_blocking_villains() {
    let mut h = OnuwTestHarness::with_extra_evil(
        &[
            ("Alice", &RAPSCALLION),
            ("Bob", &MAD_SCIENTIST),
            ("Carol", &INTERN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
        &HENCHMAN,
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "pass").await; // Solo villain LonePeek
    h.reply_dm("Alice", "Dave").await; // Rapscallion plants Henchman onto Dave!

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Henchman");
    assert!(report.dms_to("Carol").contains("The Mad Scientist is Bob"));

    // Village kills Bob (Mad Scientist) -> BOTH Mad Scientist (Bob) AND Intern (Carol) win, blocking Evil!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);
}

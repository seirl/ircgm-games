use crate::roles::base::{robber::ROBBER, troublemaker::TROUBLEMAKER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::super_villains::{
    annoying_lad::ANNOYING_LAD,
    dr_peeker::DR_PEEKER,
    evilometer::EVILOMETER,
    henchman::HENCHMAN,
    mad_scientist_intern::{INTERN, MAD_SCIENTIST},
    rapscallion::RAPSCALLION,
    rogue::ROGUE,
    role_retriever::ROLE_RETRIEVER,
    temptress::TEMPTRESS,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn rogue_waits_for_robber_and_troublemaker_then_steals_post_swap_card_and_narrates_it() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ROBBER),
            ("Bob", &TROUBLEMAKER),
            ("Carol", &ROGUE),
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
    // Bob (Troublemaker, 170) swaps Alice (now Werewolf!) and Dave (now Robber!) -> Alice=Robber, Dave=Werewolf:
    h.reply_dm("Bob", "Alice Dave").await;
    // Carol (Rogue, 175, AfterUpstreamBoard) inspects Dave (sees post-swap Werewolf!) and steals it!
    h.reply_dm("Carol", "Dave").await;
    h.reply_dm("Carol", "steal").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Carol").contains("Dave currently holds Werewolf!"));
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Rogue");

    // Verify the Dawn channel output prints the full numbered `📜 What Happened:` chronicle!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    let chan = dawn.channel_text();
    assert!(chan.contains("📜 What Happened:"));
    assert!(chan.contains("Robber (Alice) stole Werewolf from Dave (giving Dave Robber)."));
    assert!(chan.contains("Troublemaker (Bob) swapped Alice (Werewolf) and Dave (Robber)."));
    assert!(chan.contains("Rogue (Carol) stole Werewolf from Dave (giving Dave Rogue)."));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Dave"]);
}

#[tokio::test(start_paused = true)]
async fn role_retriever_notifies_victim_and_temptress_triggers_evilometer() {
    let mut h = OnuwTestHarness::with_extra_evil(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &DR_PEEKER),
            ("Carol", &ROLE_RETRIEVER),
            ("Dave", &EVILOMETER),
            ("Eve", &ANNOYING_LAD),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
        &HENCHMAN,
    )
    .await;

    let night = h.spawn_night();
    // Temptress (Alice, 128) swaps her Temptress card with Eve (seat #5, adjacent to Evilometer Dave #4!):
    h.reply_dm("Alice", "Eve").await;
    // Dr. Peeker (Bob, 131) X-rays Carol (Role Retriever):
    h.reply_dm("Bob", "Carol").await;
    // Role Retriever (Carol, 162) swaps with Alice (who now holds Annoying Lad!):
    h.reply_dm("Carol", "Alice").await;
    // Eve (started Annoying Lad, pokes left neighbor Dave):
    h.reply_dm("Eve", "left").await;

    let report = h.finish_night(night).await;
    // Alice was notified by Role Retriever that her card is now Role Retriever!
    assert!(report
        .dms_to("Alice")
        .contains("The Role Retriever swapped your card! Your card is now Role Retriever!"));
    assert!(report
        .dms_to("Carol")
        .contains("you fetched Annoying Lad!"));
    // Eve now holds Temptress (adjacent to Dave = Evilometer), so Dave's Evilometer beeps!
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

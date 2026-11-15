use crate::marks::Mark;
use crate::role::Role;
use crate::roles::base::{
    insomniac::INSOMNIAC, seer::SEER, tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::roles::daybreak::{
    paranormal_investigator::PARANORMAL_INVESTIGATOR, revealer::REVEALER,
};
use crate::roles::vampire::{gremlin::GREMLIN, marksman::MARKSMAN, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn gremlin_and_marksman_omit_marks_completely_when_no_mark_placer_is_in_deck() {
    // Intro / Werewolf-vs-Village style deck (no Vampire / Mark-placing roles in deck!):
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &GREMLIN),
            ("Bob", &INSOMNIAC),
            ("Carol", &MARKSMAN),
            ("Dave", &WEREWOLF),
        ],
        [&PARANORMAL_INVESTIGATOR, &REVEALER, &VILLAGER],
    )
    .await;

    assert!(!h.roster.deck_has_marks());
    // Deck-contextual card summaries omit Marks, Independents, and Tanners when absent from deck!
    assert!(
        !GREMLIN
            .summary_for_deck(&h.roster)
            .to_ascii_lowercase()
            .contains("mark")
    );
    assert!(
        !MARKSMAN
            .summary_for_deck(&h.roster)
            .to_ascii_lowercase()
            .contains("mark")
    );
    assert_eq!(
        PARANORMAL_INVESTIGATOR.summary_for_deck(&h.roster),
        "Look at up to 2 players' cards. If you see a Werewolf, you STOP immediately and become that role!"
    );
    assert_eq!(
        REVEALER.summary_for_deck(&h.roster),
        "Flip another player's card face up for Dawn—unless it is a Werewolf, in which case you flip it back face down!"
    );

    let night = h.spawn_night();
    // Verify initial prompts to Gremlin (Alice) and Marksman (Carol) omit Marks and seat numbers:
    let alice_prompt = h.wait_for_new_dm("Alice").await;
    assert!(!alice_prompt.to_ascii_lowercase().contains("mark"));
    assert!(alice_prompt.contains("Players: Alice, Bob, Carol, Dave"));

    // Trying `marks Alice Bob` when no Marks exist is cleanly rejected by the single `ask()` closure,
    // and numeric seat IDs (`1 2`) are rejected under nick-only targeting:
    h.send_dm("Alice", "marks Alice Bob").await;
    let reject_marks = h.wait_for_new_dm("Alice").await;
    assert!(reject_marks.contains("There are no Marks in this game"));
    h.send_dm("Alice", "1 2").await;
    let reject_num = h.wait_for_new_dm("Alice").await;
    assert!(reject_num.contains("Unknown player nickname."));

    // Dave (Lone Wolf) passes; Carol (Marksman) inspects Dave with a single `<nick>` argument;
    // Alice (Gremlin) swaps her own card with Bob using bare `<nick1> <nick2>` syntax!
    h.reply_dm("Dave", "pass").await;
    h.reply_dm("Carol", "Dave").await;
    h.send_dm("Alice", "Alice Bob").await;
    let report = h.finish_night(night).await;

    // 1. No Dusk phase banner was sent to `#onuw`!
    let chan_joined = report.channel_lines.join("\n");
    assert!(!chan_joined.contains("Dusk"));

    // 2. Marksman's prompt/result never mentions Mark tokens and uses nicknames only!
    let carol_without_role_title = report.dms_to("Carol").replace("Marksman", "");
    assert!(!carol_without_role_title.to_ascii_lowercase().contains("mark"));
    assert!(report.dms_to("Carol").contains("Dave's card is Werewolf!"));

    // 3. Gremlin swapped Alice and Bob BEFORE Insomniac (190) woke, so Bob saw Gremlin!
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Insomniac");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Gremlin");
    assert!(report.dms_to("Bob").contains("Gremlin"));
    assert!(report.has_chronicle_substr(
        "Gremlin (Alice) swapped Alice's card (Gremlin) and Bob's card (Insomniac)."
    ));
}

#[tokio::test(start_paused = true)]
async fn gremlin_enables_marks_when_mark_placer_in_deck_and_epic_battle_splits_factions() {
    // 3-Faction Epic Battle (Werewolf + Vampire + Gremlin + Seer — Marks ARE in deck!):
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &WEREWOLF),
            ("Carol", &GREMLIN),
            ("Dave", &SEER),
        ],
        [&TANNER, &VILLAGER, &VILLAGER],
    )
    .await;

    assert!(h.roster.deck_has_marks());
    assert!(GREMLIN
        .summary_for_deck(&h.roster)
        .contains("marks <nick1> <nick2>"));
    assert_eq!(
        h.roster.describe_non_village_roles(),
        "a Werewolf, Vampire, or Tanner"
    );

    let night = h.spawn_night();
    // Dusk: Vampire Alice marks Dave.
    h.reply_dm("Alice", "Dave").await;
    // Night: Lone Werewolf Bob peeks Center #2; Seer Dave peeks Alice; Gremlin Carol swaps Marks!
    h.reply_dm("Bob", "2").await;
    h.reply_dm("Dave", "Alice").await;
    h.reply_dm("Carol", "marks Dave Carol").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Vampire);
    assert_eq!(report.mark_at(&h.roster, "Dave"), Mark::Clarity);
    assert!(report.has_chronicle_substr(
        "Gremlin (Carol) swapped Dave's Mark (Mark of the Vampire) and Carol's Mark (Mark of Clarity)."
    ));

    // Epic Battle Dawn: Alice (Vampire) and Dave (Seer) tie at 2 votes and both die, while Bob (Werewolf) survives!
    // Because Bob's Werewolf faction had 0 deaths AND a rival Vampire died, ONLY Werewolves (Bob) win — Village loses because a Werewolf survived!
    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob"]);
}

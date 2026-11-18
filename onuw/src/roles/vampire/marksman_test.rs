use crate::roles::base::{doppelganger::DOPPELGANGER, villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::alpha_wolf::ALPHA_WOLF;
use crate::roles::vampire::{marksman::MARKSMAN, vampire::VAMPIRE};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn marksman_and_doppelganger_marksman_inspect_card_and_dusk_mark() {
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
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Carol", "Dave").await;
    h.reply_dm("Bob", "Carol Dave").await;
    h.reply_dm("Alice", "Dave Carol").await;

    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Bob")
        .contains("Carol's card is Vampire, and Dave's mark is Mark of the Vampire!"));
    assert!(report
        .dms_to("Alice")
        .contains("Dave's card is Villager, and Carol's mark is Mark of Clarity!"));
}

#[tokio::test(start_paused = true)]
async fn marksman_allows_own_card_or_own_mark_while_rejecting_same_player_for_both() {
    // 1. Pre-#5-D Alpha Wolf swap onto Marksman: Marksman can inspect own card + another's Mark,
    //    while `<same> <same>` is rejected:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &MARKSMAN),
            ("Carol", &VAMPIRE),
            ("Dave", &VILLAGER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "Dave").await; // Dusk #-6: Vampire marks Dave
    h.reply_dm("Alice", "pass").await; // Night #2: Lone Wolf center peek
    h.reply_dm("Alice", "Bob").await; // Night #2-B: Alpha Wolf swaps Center Werewolf onto Bob!

    let prompt = h.wait_for_prompt_dm("Bob").await;
    assert!(prompt.contains("Marksman:"));

    h.send_dm("Bob", "Bob Bob").await;
    let rej_self = h.wait_for_new_dm("Bob").await;
    assert!(rej_self.contains("DIFFERENT players"), "got: {rej_self}");

    h.send_dm("Bob", "Dave Dave").await;
    let rej_other = h.wait_for_new_dm("Bob").await;
    assert!(rej_other.contains("DIFFERENT players"), "got: {rej_other}");

    h.send_dm("Bob", "Bob Dave").await;
    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Bob")
        .contains("Bob's card is Werewolf, and Dave's mark is Mark of the Vampire!"));

    // 2. Inspecting another player's card + own Mark (`<other> <self>`), `mark <self>`, and `card <self>`:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MARKSMAN),
            ("Carol", &VAMPIRE),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    h2.reply_dm("Carol", "Bob").await; // Vampire bites Bob!
    h2.reply_dm("Bob", "Carol Bob").await; // Carol's card + Bob's own Mark
    h2.reply_dm("Alice", "card Alice").await; // Doppelgänger-Marksman checks own card only
    let rep2 = h2.finish_night(n2).await;
    assert!(rep2
        .dms_to("Bob")
        .contains("Carol's card is Vampire, and Bob's mark is Mark of the Vampire!"));
    assert!(rep2
        .dms_to("Alice")
        .contains("Alice's card is Doppelgänger!"));

    // 3. Markless deck: Marksman can view own card (`<self>`):
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &MARKSMAN),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n3 = h3.spawn_night();
    h3.reply_dm("Alice", "Alice").await;
    let rep3 = h3.finish_night(n3).await;
    assert!(rep3.dms_to("Alice").contains("Alice's card is Marksman!"));
}

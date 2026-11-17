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

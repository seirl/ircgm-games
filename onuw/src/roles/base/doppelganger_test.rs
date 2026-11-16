use crate::roles::alien::oracle::ORACLE;
use crate::roles::base::{
    doppelganger::DOPPELGANGER, insomniac::INSOMNIAC, minion::MINION, robber::ROBBER, seer::SEER,
    villager::VILLAGER, werewolf::WEREWOLF,
};
use crate::roles::daybreak::{alpha_wolf::ALPHA_WOLF, mystic_wolf::MYSTIC_WOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn doppelganger_copies_robber_acts_immediately_and_chronicle_records_both_steps() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &ROBBER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice (Doppelgänger) copies Bob (Robber), then immediately steals Carol's Werewolf card:
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;
    // Meanwhile Bob (the original Robber) passes:
    h.reply_dm("Bob", "pass").await;
    // Carol (Lone Wolf) passes center peek:
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Robber");
    assert!(report.has_chronicle_substr("Doppelgänger (Alice) copied Bob and became Robber."));
    assert!(report.has_chronicle_substr(
        "Doppelgänger-Robber (Alice) stole Werewolf from Carol (giving Carol Doppelgänger)."
    ));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_insomniac_wakes_at_tail_of_night_and_sees_swapped_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &INSOMNIAC),
            ("Carol", &ROBBER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Carol (Robber) steals Alice's card while Alice copies Bob (Insomniac):
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Carol", "Alice").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Robber");
    // Carol stole the physical Doppelgänger card (which had copied Insomniac), so Carol's effective role is Insomniac,
    // while Carol saw the physical "Doppelgänger" face when robbing!
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Insomniac");
    assert!(report.dms_to("Carol").contains("got: Doppelgänger!"));
    // Alice (Doppelgänger-Insomniac) wakes after all mutations and sees she now holds Robber:
    assert!(report
        .dms_to("Alice")
        .contains("Doppelgänger-Insomniac: You check your card before dawn — it is Robber!"));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_werewolf_joins_pack_wake_and_is_seen_by_minion() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &WEREWOLF),
            ("Carol", &MINION),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Fellow Werewolf pack: Bob"));
    assert!(report.dms_to("Bob").contains("Fellow Werewolf pack: Alice"));
    assert!(report
        .dms_to("Carol")
        .contains("The Werewolves are Alice, Bob"));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_mystic_wolf_peeks_immediately_then_joins_pack_huddle() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &MYSTIC_WOLF),
            ("Carol", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // #1: Alice (Doppelgänger) copies Bob (Mystic Wolf), then IMMEDIATELY peeks Carol's card at #1
    // (before the Werewolf pack huddle at #2), and stays asleep during Bob's #2-C Mystic Wolf turn:
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Alice", "Carol").await;
    // #2-C / #5: Bob (Mystic Wolf) peeks Carol and Carol (Seer) peeks Bob:
    h.reply_dm("Bob", "Carol").await;
    h.reply_dm("Carol", "Bob").await;

    let report = h.finish_night(night).await;
    let alice_plain = report.dms_to("Alice");
    let peek_pos = alice_plain
        .find("Mystic Wolf: Reply")
        .expect("Alice got immediate Mystic Wolf peek prompt at #1");
    let pack_pos = alice_plain
        .find("Fellow Werewolf pack: Bob")
        .expect("Alice woke in Werewolf pack huddle at #2");
    assert!(
        peek_pos < pack_pos,
        "Doppelgänger-Mystic Wolf must peek immediately at #1 before joining the #2 Werewolf huddle"
    );
    assert!(alice_plain.contains("Carol's card is: Seer"));
    assert!(report.has_chronicle_substr("Doppelgänger-Mystic Wolf (Alice) viewed Carol's card (Seer)."));
    assert!(report.has_chronicle_substr("Mystic Wolf (Bob) viewed Carol's card (Seer)."));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_alpha_wolf_swaps_immediately_without_pack_leak_before_real_alpha_wolf() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &ALPHA_WOLF),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
            ("Eve", &SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // #1: Alice copies Bob (Alpha Wolf). In her immediate #1 prompt, she only knows herself and Bob
    // (Carol's Werewolf card is NOT leaked before #2!), and swaps the Center Werewolf onto Dave
    // (putting Dave's Villager card into CenterWolfCard):
    h.reply_dm("Alice", "Bob").await;
    let imm_prompt = loop {
        let m = h.wait_for_new_dm("Alice").await;
        if m.contains("Alpha Wolf:") {
            break m;
        }
    };
    assert!(
        imm_prompt.contains("Carol, Dave, Eve"),
        "pre-pack #1 prompt must not leak Carol as a Werewolf: {imm_prompt}"
    );
    h.send_dm("Alice", "Dave").await;

    // #2-B: After the #2 pack huddle (Alice, Bob, Carol), ONLY Bob wakes as Alpha Wolf (seeing only
    // Dave & Eve as non-Wolves!) and swaps CenterWolfCard (now Dave's old Villager!) onto Eve, while Eve peeks Bob:
    let bob_prompt = loop {
        let m = h.wait_for_new_dm("Bob").await;
        if m.contains("Alpha Wolf:") {
            break m;
        }
    };
    assert!(
        bob_prompt.contains("Non-Wolves: Dave, Eve") && !bob_prompt.contains("Carol"),
        "post-pack #2-B prompt excludes all waking packmates: {bob_prompt}"
    );
    h.send_dm("Bob", "Eve").await;
    h.reply_dm("Eve", "Bob").await;

    let report = h.finish_night(night).await;
    // Dave keeps the Werewolf card planted by Doppelgänger-Alpha Wolf at #1, while Eve received
    // Dave's displaced Villager card when the real Alpha Wolf swapped at #2-B!
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Villager");
    assert!(report.dms_to("Alice").contains("Fellow Werewolf pack: Bob, Carol"));
}

#[tokio::test(start_paused = true)]
async fn doppelganger_copying_converted_oracle_wakes_with_aliens_without_rerunning_oracle() {
    use crate::roles::alien::{
        alien::{AlienEvent, ForcedAlienEvent, ALIEN},
        oracle::{ForcedOracleQuestion, OracleQuestion},
    };
    use crate::role::NightFaction;

    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &ORACLE),
            ("Carol", &ALIEN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedOracleQuestion(Some(OracleQuestion::ChangeTeam(
        NightFaction::Alien,
    ))));
    h.set_key(ForcedAlienEvent(Some(AlienEvent::CenterProbe)));

    let night = h.spawn_night();
    // Oracle Bob (#-9 / order 1) accepts joining the Alien team:
    h.reply_dm("Bob", "yes").await;
    // Doppelgänger Alice (#1) copies Bob (Oracle [Alien]) — does NOT re-run #-9, instead wakes with Aliens at #1A!
    h.reply_dm("Alice", "Bob").await;
    // At #1A, Alice, Bob, and Carol all wake together in [Aliens]; Carol probes Center #1:
    h.reply_dm("Carol", "!probe 1").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Aliens aboard: Alice, Bob, Carol"));
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Oracle [Alien]");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Oracle [Alien]");
}

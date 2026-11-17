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
async fn doppelganger_mystic_wolf_peeks_immediately_then_joins_pack_wake() {
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
    // (before the Werewolf pack wake at #2), and stays asleep during Bob's #2-C Mystic Wolf turn:
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
        .expect("Alice woke in Werewolf pack wake at #2");
    assert!(
        peek_pos < pack_pos,
        "Doppelgänger-Mystic Wolf must peek immediately at #1 before joining the #2 Werewolf wake"
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

    // #2-B: After the #2 pack wake (Alice, Bob, Carol), ONLY Bob wakes as Alpha Wolf (seeing only
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
async fn doppelganger_oracle_answers_second_question_and_copying_another_doppelganger_sleeps() {
    use crate::role::{NightFaction, Team};
    use crate::roles::alien::{
        alien::{AlienEvent, ForcedAlienEvent, ALIEN},
        oracle::{ForcedDoppelgangerOracleQuestion, ForcedOracleQuestion, OracleQuestion},
    };
    use ircgm::prelude::Text;

    // 1. Oracle Bob (#-9) answers EvenOdd (stays Village Oracle), while Doppelgänger Alice (#1)
    //    copies Bob and wakes in `Doppelgänger-Oracle` to answer her OWN ChangeTeam(Alien) question!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &ORACLE),
            ("Carol", &ALIEN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedOracleQuestion(Some(OracleQuestion::EvenOdd)));
    h.set_key(ForcedDoppelgangerOracleQuestion(Some(
        OracleQuestion::ChangeTeam(NightFaction::Alien),
    )));
    h.set_key(ForcedAlienEvent(Some(AlienEvent::CenterProbe)));

    let night = h.spawn_night();
    h.reply_dm("Bob", "even").await;
    h.reply_dm("Alice", "Bob").await;
    // Alice wakes as Doppelgänger-Oracle right after Doppelgänger (before Aliens at #1A!):
    h.reply_dm("Alice", "yes").await;
    // At #1A, Alice and Carol wake together as Aliens (Bob stayed Village!):
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Carol").await;
    h.send_dm("Alice", "1").await;
    h.send_dm("Carol", "1").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Aliens aboard: Alice, Carol"));
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Oracle [Alien]");
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Oracle");
    assert!(report.channel_lines.iter().any(|l| {
        let p = Text::parse(l).plain();
        p.contains("Oracle Omen:") && p.contains("EVEN")
    }));
    assert!(report.channel_lines.iter().any(|l| {
        let p = Text::parse(l).plain();
        p.contains("Doppelgänger-Oracle Omen:") && p.contains("Alien")
    }));

    // 2. `doppelganger*2`: Alice (seat 0, Doppelgänger) copies Carol (Werewolf) while Bob (seat 1, Doppelgänger)
    //    copies Alice (seat 0) — Bob MUST sleep as a Village Doppelgänger regardless of seat resolution order!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &DOPPELGANGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.wait_for_prompt_dm("Alice").await;
    h2.wait_for_prompt_dm("Bob").await;
    h2.send_dm("Alice", "Carol").await;
    h2.send_dm("Bob", "Alice").await;
    let rep2 = h2.finish_night(n2).await;

    assert_eq!(rep2.board.effective_team(0), Team::Werewolf);
    assert_eq!(rep2.board.effective_team(1), Team::Village);
    assert!(rep2.dms_to("Bob").contains("remain on the Village team"));
    assert!(!rep2.dms_to("Bob").contains("Fellow Werewolf pack"));
}

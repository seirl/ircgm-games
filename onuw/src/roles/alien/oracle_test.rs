use ircgm::prelude::Text;

use crate::role::NightFaction;
use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    groob_zerb::GROOB,
    oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE},
};
use crate::roles::base::{drunk::DRUNK, seer::SEER, tanner::TANNER, villager::VILLAGER};
use crate::roles::daybreak::{
    paranormal_investigator::PARANORMAL_INVESTIGATOR, revealer::REVEALER, witch::WITCH,
};
use crate::roles::vampire::copycat::COPYCAT;
use crate::testing::{NightReport, OnuwTestHarness};

fn clone_report(r: &NightReport) -> NightReport {
    NightReport {
        board: r.board.clone(),
        chronicle: r.chronicle.clone(),
        chronicle_plain: r.chronicle_plain.clone(),
        dms_by_nick: r.dms_by_nick.clone(),
        timed_dms: r.timed_dms.clone(),
        timed_channel: r.timed_channel.clone(),
        channel_lines: r.channel_lines.clone(),
        total_elapsed: r.total_elapsed,
    }
}

#[tokio::test(start_paused = true)]
async fn oracle_center_conversions_satisfy_4c_and_4d_with_copycat_and_drunk() {
    let mut h_4c = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &DRUNK),
            ("Carol", &TANNER),
            ("Dave", &ALIEN),
        ],
        [&ORACLE, &VILLAGER, &VILLAGER],
    )
    .await;
    h_4c.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 99,
    })));
    h_4c.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));

    let n_4c = h_4c.spawn_night();
    h_4c.reply_dm("Alice", "1").await;
    h_4c.reply_dm("Dave", "pass").await;
    h_4c.reply_dm("Bob", "1").await;
    let rep_4c = h_4c.finish_night(n_4c).await;

    assert_eq!(rep_4c.effective_role_at(&h_4c.roster, "Alice"), "Oracle");
    assert_eq!(
        rep_4c.effective_role_at(&h_4c.roster, "Bob"),
        "Oracle [Hunted]"
    );

    let dawn_survives = h_4c
        .resolve_voting(
            clone_report(&rep_4c),
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn_survives.winner_nicks(&h_4c.roster), vec!["Bob"]);

    let dawn_slain = h_4c
        .resolve_voting(
            rep_4c,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(
        dawn_slain.winner_nicks(&h_4c.roster),
        vec!["Alice", "Carol", "Dave"]
    );

    let mut h_4d = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &WITCH),
            ("Carol", &COPYCAT),
            ("Dave", &DRUNK),
            ("Eve", &ALIEN),
        ],
        [&VILLAGER, &ALIEN, &VILLAGER],
    )
    .await;
    h_4d.set_key(ForcedOracleQuestion(Some(OracleQuestion::AlienExchange {
        symp_on_no: true,
    })));
    h_4d.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));

    let n_4d = h_4d.spawn_night();
    h_4d.reply_dm("Alice", "no").await;
    h_4d.reply_dm("Carol", "1").await;
    h_4d.reply_dm("Eve", "pass").await;
    h_4d.reply_dm("Bob", "1").await;
    h_4d.reply_dm("Bob", "Alice").await;
    h_4d.reply_dm("Dave", "1").await;
    let rep_4d = h_4d.finish_night(n_4d).await;

    assert!(!rep_4d.dms_to("Alice").contains("Aliens aboard"));
    assert_eq!(
        rep_4d.effective_role_at(&h_4d.roster, "Dave"),
        "Oracle [Alien Team]"
    );

    let dawn_4d = h_4d
        .resolve_voting(
            rep_4d,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Dave")),
            ],
        )
        .await;
    assert_eq!(dawn_4d.winner_nicks(&h_4d.roster), vec!["Dave", "Eve"]);
}

#[tokio::test(start_paused = true)]
async fn oracle_alien_exchange_jackpot_wake_log_player_peek_and_public_conversion_work() {
    // 1. AlienExchange = yes forces waking Aliens to swap cards:
    let mut h1 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &ALIEN),
            ("Carol", &GROOB),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h1.set_key(ForcedOracleQuestion(Some(OracleQuestion::AlienExchange {
        symp_on_no: false,
    })));
    let n1 = h1.spawn_night();
    h1.reply_dm("Alice", "yes").await;
    let rep1 = h1.finish_night(n1).await;
    assert_eq!(rep1.effective_role_at(&h1.roster, "Bob"), "Groob");
    assert_eq!(rep1.effective_role_at(&h1.roster, "Carol"), "Alien");

    // 2. NumberGuess jackpot delivers wake log DM:
    let mut h2 = OnuwTestHarness::new(
        &[("Alice", &ORACLE), ("Bob", &SEER), ("Carol", &DRUNK)],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 7,
    })));
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "7").await;
    h2.reply_dm("Bob", "Carol").await;
    h2.reply_dm("Carol", "1").await;
    let rep2 = h2.finish_night(n2).await;
    let alice_watch = Text::parse(&rep2.dms_to("Alice")).plain();
    assert!(
        alice_watch.contains("Oracle All-Night Watch")
            && alice_watch.contains("Seer: Bob")
            && alice_watch.contains("Drunk: Carol")
    );

    // 3. PlayerPeek with redirect + public channel announcement:
    let mut h3 = OnuwTestHarness::new(
        &[("Alice", &ORACLE), ("Bob", &SEER), ("Carol", &VILLAGER)],
        [&TANNER, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedOracleQuestion(Some(OracleQuestion::PlayerPeek {
        redirect: Some(1),
    })));
    let n3 = h3.spawn_night();
    h3.reply_dm("Alice", "Carol").await;
    h3.reply_dm("Bob", "pass").await;
    let rep3 = h3.finish_night(n3).await;
    assert!(rep3.dms_to("Alice").contains("Bob holds Seer"));
    assert!(rep3
        .channel_lines
        .iter()
        .any(|l| Text::parse(l).plain().contains("Bob's card was viewed by the Oracle")));

    // 4. Publicly converted Oracle triggers PI & Revealer flip-back:
    let mut h4 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &PARANORMAL_INVESTIGATOR),
            ("Carol", &REVEALER),
            ("Dave", &ALIEN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h4.set_key(ForcedOracleQuestion(Some(OracleQuestion::ChangeTeam(
        NightFaction::Alien,
    ))));
    h4.set_key(ForcedAlienEvent(Some(AlienEvent::CenterProbe)));
    let n4 = h4.spawn_night();
    h4.reply_dm("Alice", "yes").await;
    h4.reply_dm("Dave", "!pass").await;
    h4.reply_dm("Bob", "Alice").await;
    h4.reply_dm("Carol", "Alice").await;
    let rep4 = h4.finish_night(n4).await;
    assert_eq!(
        rep4.effective_role_at(&h4.roster, "Bob"),
        "Oracle [Alien]"
    );
    assert!(!rep4.board.is_player_face_up(h4.seat_of("Alice")));
}

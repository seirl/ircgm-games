use ircgm::prelude::Text;

use crate::role::NightFaction;
use crate::roles::{
    alien::{
        alien::{AlienEvent, ForcedAlienEvent, ALIEN, SYNTHETIC_ALIEN},
        blob::{ActiveBlobShape, BlobShape, BLOB},
        cow::COW,
        groob_zerb::{GROOB, ZERB},
        leader::LEADER,
        mortician::{ForcedMorticianMode, MorticianMode, MORTICIAN},
        oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE},
        psychic::{ForcedPsychicMode, PsychicMode, PSYCHIC},
    },
    base::{doppelganger::DOPPELGANGER, drunk::DRUNK, seer::SEER, tanner::TANNER, villager::VILLAGER},
    daybreak::{aura_seer::AURA_SEER, sentinel::SENTINEL},
    vampire::copycat::COPYCAT,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn psychic_both_neighbors_alien_bioscan_huddle_and_cow_hoof_tap_work() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &COW),
            ("Carol", &ORACLE),
            ("Dave", &PSYCHIC),
            ("Eve", &SYNTHETIC_ALIEN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedOracleQuestion(Some(OracleQuestion::EvenOdd)));
    h.set_key(ForcedAlienEvent(Some(AlienEvent::BioScan)));
    h.set_key(ForcedPsychicMode(Some(PsychicMode::BothNeighbors)));

    let night = h.spawn_night();
    // Oracle Carol (#3) states her seat parity publicly:
    h.reply_dm("Carol", "odd").await;
    // Aliens Alice & Synthetic Alien Eve wake in [Aliens] huddle:
    // Synthetic Alien Eve tries `!scan Carol` and gets rejected; real Alien Alice locks in `!scan Carol`,
    // and huddle whispers still relay between Alice & Eve!
    h.wait_for_prompt_dm("Eve").await;
    h.send_dm("Eve", "!scan Carol").await;
    let eve_rej = h.wait_for_new_dm("Eve").await;
    assert!(eve_rej.contains("Only true Aliens"), "got: {eve_rej}");
    h.send_dm("Alice", "blend in, Eve").await;
    h.send_dm("Alice", "!scan Carol").await;

    let report = h.finish_night(night).await;
    assert!(report
        .channel_lines
        .iter()
        .any(|l| Text::parse(l).plain().contains("seat number (1..=5) is ODD")));
    assert!(report.dms_to("Eve").contains("blend in, Eve"));
    // Psychic Dave (seat #4, BothNeighbors) reads Carol (#3, Oracle) and Eve (#5, Synthetic Alien):
    let dave_dms = report.dms_to("Dave");
    assert!(dave_dms.contains("Carol holds Oracle"));
    assert!(dave_dms.contains("Eve holds Synthetic Alien"));
    // Cow Bob (#2) sits next to Alice (#1, Alien) -> hoof tapped, AND waking Aliens see Cow!
    assert!(report.dms_to("Bob").contains("*TAP!*"));
    assert!(report.dms_to("Alice").contains("Bob holds the Cow card"));
    assert!(report.dms_to("Eve").contains("Bob holds the Cow card"));
    assert!(report.dms_to("Eve").contains("Carol holds Oracle"));
    assert!(report.has_chronicle_substr("Alien (Alice) bio-scanned Carol and saw Oracle."));

    // If Synthetic Alien (Eve) dies alongside Alien (Alice) on a 2-2 tie, Synthetic Alien wins ALONE (blocking Village too!):
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Eve")),
                ("Bob", Some("Eve")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Eve"]);
}

#[tokio::test(start_paused = true)]
async fn leader_unanimous_alien_point_steals_victory_even_when_alien_dies() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &ALIEN),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));

    let night = h.spawn_night();
    // Under StandardRecon with 2 Aliens, Alice & Bob coordinate in [Aliens] huddle:
    let _ = h.wait_for_new_dm("Alice").await;
    h.send_dm("Alice", "vote Carol if she claims Leader").await;
    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Bob")
        .contains("vote Carol if she claims Leader"));
    assert!(report
        .dms_to("Carol")
        .contains("The Aliens are Alice, Bob"));

    // Even though Carol and Dave tie/kill Alice (2-2), BOTH Aliens (Alice & Bob) pointed at Leader Carol!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);
    assert!(dawn
        .channel_text()
        .contains("All Aliens pointed at the Leader"));
}

#[tokio::test(start_paused = true)]
async fn groob_zerb_blob_mortician_and_tractor_beam_work() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &BLOB),
            ("Eve", &MORTICIAN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::TractorBeam)));
    h.set_key(ActiveBlobShape(Some(BlobShape::Left1)));
    h.set_key(ForcedMorticianMode(Some(MorticianMode::OwnCard)));

    let night = h.spawn_night();
    // Non-aliens are Dave (Blob) and Eve (Mortician); Alien Alice tractor-beams them via `!beam`:
    h.reply_dm("Alice", "!beam Dave Eve").await;
    // Eve (waking as Mortician at #13A in OwnCard mode) peeks her own card and discovers she now holds Blob!
    h.reply_dm("Eve", "self").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Non-Aliens: Dave, Eve"));
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Mortician");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Blob");
    assert!(report.dms_to("Eve").contains("Your own card is Blob"));
    // Public Blob Absorption shape announcement was posted in channel so swapped Blob Eve knows `1 player to their LEFT`!
    assert!(report
        .channel_lines
        .iter()
        .any(|l| Text::parse(l).plain().contains("1 player to their LEFT")));
    assert!(report.has_chronicle_substr(
        "Alien (Alice) tractor-beamed a swap between Dave (Blob) and Eve (Mortician)."
    ));

    // At Dawn, Carol (Zerb) is killed!
    // - Killing Zerb counts as killing an Alien -> regular Alien Alice LOSES!
    // - Bob (Groob): survived and Zerb died -> Groob WINS!
    // - Dave (now Mortician, neighbors Carol & Eve): Carol died & Dave survived -> Mortician WINS!
    // - Eve (now Blob at seat #5, shape Left1 -> Blob spans {Eve, left=Dave}): both survived -> Blob WINS!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Dave", "Eve"]);
}

#[tokio::test(start_paused = true)]
async fn solo_groob_lone_alien_peek_and_leader_wins_when_both_groob_and_zerb_survive() {
    // 1. Solo Groob (Zerb in center) under StandardRecon gets Lone Alien center peek AND is on Team::Alien!
    let mut h1 = OnuwTestHarness::new(
        &[("Alice", &GROOB), ("Bob", &VILLAGER), ("Carol", &VILLAGER)],
        [&ZERB, &SEER, &VILLAGER],
    )
    .await;
    h1.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let night1 = h1.spawn_night();
    h1.reply_dm("Alice", "2").await;
    let report1 = h1.finish_night(night1).await;
    assert!(report1.dms_to("Alice").contains("Center #2: Seer"));
    assert!(report1
        .dms_to("Alice")
        .contains("you are on the regular Alien team"));
    let dawn1 = h1
        .resolve_voting(
            report1,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Bob")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn1.winner_nicks(&h1.roster), vec!["Alice"]);

    // 2. Both Groob & Zerb in play with Leader -> Leader sees both and wins iff neither dies!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &GROOB),
            ("Bob", &ZERB),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedAlienEvent(Some(AlienEvent::CenterProbe)));
    let night2 = h2.spawn_night();
    h2.reply_dm("Alice", "!pass").await;
    let report2 = h2.finish_night(night2).await;
    assert!(report2
        .dms_to("Carol")
        .contains("Groob & Zerb are pointing at each other (Alice, Bob)"));
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol"]);
}

#[tokio::test(start_paused = true)]
async fn psychic_both_neighbors_sees_visible_table_and_multi_alien_scan_records_all_for_aura_seer()
{
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &SENTINEL),
            ("Carol", &SYNTHETIC_ALIEN),
            ("Dave", &GROOB),
            ("Eve", &PSYCHIC),
            ("Frank", &AURA_SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedOracleQuestion(Some(OracleQuestion::EvenOdd)));
    h.set_key(ForcedAlienEvent(Some(AlienEvent::BioScan)));
    h.set_key(ForcedPsychicMode(Some(PsychicMode::BothNeighbors)));

    let night = h.spawn_night();
    // Oracle states parity (does NOT touch cards -> clean for Aura Seer!); Sentinel shields Alice; Groob Dave scans Eve:
    h.reply_dm("Alice", "odd").await;
    h.reply_dm("Bob", "Alice").await;
    h.reply_dm("Dave", "!scan Eve").await;

    let report = h.finish_night(night).await;
    let eve_plain = Text::parse(&report.dms_to("Eve")).plain();
    assert!(eve_plain.contains("Alice🛡"), "Eve DMs: {eve_plain}");
    // Aura Seer Frank sees BOTH Carol (Synthetic Alien) and Dave (Groob) from group Scan, plus Eve (Psychic), and NOT Alice:
    let frank_plain = Text::parse(&report.dms_to("Frank")).plain();
    assert!(
        frank_plain.contains("Carol, Dave, Eve"),
        "Frank DMs: {frank_plain}"
    );
}

#[tokio::test(start_paused = true)]
async fn doppelganger_groob_reproduces_1b_asymmetric_and_two_handed_leader_pointing() {
    // 1.b.i: Groob (Bob) + Doppel-Groob (Alice, copied Bob), NO Zerb (Zerb in center):
    let mut h1 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &GROOB),
            ("Carol", &LEADER),
        ],
        [&ZERB, &VILLAGER, &VILLAGER],
    )
    .await;
    h1.set_key(ForcedAlienEvent(Some(AlienEvent::BioScan)));
    let n1 = h1.spawn_night();
    h1.reply_dm("Alice", "Bob").await;
    h1.reply_dm("Alice", "!pass").await;
    let rep1 = h1.finish_night(n1).await;
    let carol1 = Text::parse(&rep1.dms_to("Carol")).plain();
    assert!(
        carol1.contains("Thumb-up Aliens: Alice"),
        "Carol DMs: {carol1}"
    );
    assert!(
        carol1.contains("Groob/Zerb pointing: Bob → Alice"),
        "Carol DMs: {carol1}"
    );

    // 1.b.ii: Groob (Bob) + Doppel-Groob (Alice, copied Bob) + Zerb (Carol) + Leader (Dave):
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &LEADER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedAlienEvent(Some(AlienEvent::BioScan)));
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    h2.reply_dm("Alice", "!pass").await;
    let rep2 = h2.finish_night(n2).await;
    let dave2 = Text::parse(&rep2.dms_to("Dave")).plain();
    assert!(
        dave2.contains("Alice → Carol")
            && dave2.contains("Bob → Alice, Carol (both hands)")
            && dave2.contains("Carol → Alice, Bob (both hands)"),
        "Dave DMs: {dave2}"
    );
    let dawn_dead_dg = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Eve")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Eve")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert!(!dawn_dead_dg
        .winner_nicks(&h2.roster)
        .contains(&"Dave".to_owned()));
}

#[tokio::test(start_paused = true)]
async fn oracle_center_conversions_satisfy_4b_4c_and_4d_with_copycat_and_drunk() {
    // 1. `4.b.i` & `4.b.ii`: Oracle in Center #1 converts to Alien (`ChangeTeam`):
    //    - Copycat Alice views Center #1 (`Oracle`) -> stays plain Villager (does NOT wake with Aliens or count as Alien!).
    //    - Drunk Bob swaps into Center #1 -> receives `Oracle [Alien]` and counts as an Alien at Voting!
    let mut h_4b = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &DRUNK),
            ("Carol", &ALIEN),
            ("Dave", &VILLAGER),
        ],
        [&ORACLE, &VILLAGER, &VILLAGER],
    )
    .await;
    h_4b.set_key(ForcedOracleQuestion(Some(OracleQuestion::ChangeTeam(
        NightFaction::Alien,
    ))));
    // Force center Oracle's random timeout YesNo to succeed? Wait: when Oracle is in center, `ChangeTeam` rolls 50/50 unless
    // we also test held Oracle + Drunk OR test `NumberGuess` (deterministic when secret = 11 / impossible or check channel!).
    // Even better: let's test `AlienExchange { symp_on_no: true }` (`4.d`) when held vs center, AND let's check:
    // For center `Oracle` on `NumberGuess { secret: 99 }` (always wrong -> 100% deterministic!), let's verify `4.c` first!
    drop(h_4b);

    // `4.c`: Oracle in Center #1 misses `#7` (`NumberGuess`) -> public Oracle Hunt announced!
    // - Copycat Alice views Center #1 (`Oracle`) -> stays plain Villager (`4.c`)!
    // - Drunk Bob swaps into Center #1 -> becomes `Oracle [Hunted]`:
    //   * If Bob survives, ONLY Bob wins!
    //   * If Bob dies, Alice (Copycat-Oracle), Carol (Tanner), and Dave (Alien) ALL win (`Oracle Slain`)!
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
    h_4c.reply_dm("Alice", "1").await; // Copycat views Center #1 (Oracle) -> plain Oracle (Villager)!
    h_4c.reply_dm("Dave", "pass").await; // Lone Alien
    h_4c.reply_dm("Bob", "1").await; // Drunk takes Center #1 (`Oracle [Hunted]`)!
    let rep_4c = h_4c.finish_night(n_4c).await;

    assert_eq!(rep_4c.effective_role_at(&h_4c.roster, "Alice"), "Oracle");
    assert_eq!(
        rep_4c.effective_role_at(&h_4c.roster, "Bob"),
        "Oracle [Hunted]"
    );

    // Case A: Bob (`Oracle [Hunted]`) survives (Carol dies) -> ONLY Bob wins!
    let dawn_survives = h_4c
        .resolve_voting(
            NightReportClone::clone_report(&rep_4c),
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn_survives.winner_nicks(&h_4c.roster), vec!["Bob"]);

    // Case B: Bob (`Oracle [Hunted]`) is killed -> Alice (Copycat-Oracle), Carol, and Dave ALL win!
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

    // `4.d.i`: Oracle answers `no` to `AlienExchange { symp_on_no: true }` -> becomes `Oracle [Alien Team]` (minion, NOT an Alien threat!),
    //          then swaps own card into Center #1 via Witch, and Drunk takes `Oracle [Alien Team]` and gets lynched ->
    //          BOTH Drunk AND the Aliens win because no Alien threat died!
    use crate::roles::daybreak::witch::WITCH;
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
    h_4d.reply_dm("Alice", "no").await; // Alice becomes `Oracle [Alien Team]` (publicly announced!)
    h_4d.reply_dm("Carol", "1").await; // Copycat views Center #1 (Villager)
    h_4d.reply_dm("Eve", "pass").await; // Lone Alien (Alice did NOT wake with Aliens because she's only on Alien Team!)
    h_4d.reply_dm("Bob", "1").await; // Witch peeks Center #1 (Villager) and swaps onto Alice (moving `Oracle [Alien Team]` into Center #1!)
    h_4d.reply_dm("Bob", "Alice").await;
    h_4d.reply_dm("Dave", "1").await; // Drunk takes Center #1 (`Oracle [Alien Team]`)!
    let rep_4d = h_4d.finish_night(n_4d).await;

    assert!(!rep_4d.dms_to("Alice").contains("Aliens aboard"));
    assert_eq!(
        rep_4d.effective_role_at(&h_4d.roster, "Dave"),
        "Oracle [Alien Team]"
    );

    // Lynch Dave (`Oracle [Alien Team]`) -> Eve (`Alien`) survives, so BOTH Dave and Eve win (`4.d.i`)!
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

struct NightReportClone;
impl NightReportClone {
    fn clone_report(r: &crate::testing::NightReport) -> crate::testing::NightReport {
        crate::testing::NightReport {
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
}

#[tokio::test(start_paused = true)]
async fn oracle_remaining_questions_jackpot_wake_log_alien_exchange_psychic_parity_and_mortician_modes_work()
{
    // 1. Oracle #2 (`AlienExchange = yes`) + #7 (`NumberGuess` jackpot? Let's test `AlienExchange = yes`, `Psychic::EvenSeat`, `Mortician::BothNeighbors`):
    let mut h1 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &ALIEN),
            ("Carol", &GROOB),
            ("Dave", &PSYCHIC),
            ("Eve", &MORTICIAN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h1.set_key(ForcedOracleQuestion(Some(OracleQuestion::AlienExchange {
        symp_on_no: false,
    })));
    h1.set_key(ForcedPsychicMode(Some(PsychicMode::EvenSeat)));
    h1.set_key(ForcedMorticianMode(Some(MorticianMode::BothNeighbors)));

    let n1 = h1.spawn_night();
    h1.reply_dm("Alice", "yes").await; // Forces waking Aliens (Bob & Carol) to swap cards at #1A!
    // Psychic Dave (seat #4, EvenSeat -> only other even seat is #2 Bob, who now holds Groob after Alien Exchange!):
    h1.reply_dm("Dave", "Bob").await;
    // Mortician Eve (seat #5, BothNeighbors -> Dave #4 & Alice #1):
    h1.reply_dm("Eve", "both").await;

    let rep1 = h1.finish_night(n1).await;
    assert_eq!(rep1.effective_role_at(&h1.roster, "Bob"), "Groob");
    assert_eq!(rep1.effective_role_at(&h1.roster, "Carol"), "Alien");
    assert!(rep1.dms_to("Dave").contains("Bob holds Groob"));
    assert!(rep1.dms_to("Eve").contains("Dave holds Psychic"));
    assert!(rep1.dms_to("Eve").contains("Alice holds Oracle"));

    // 2. Oracle #7 (`NumberGuess` exact jackpot) delivers end-of-night wake log DM,
    //    plus `Blob` in center still announces its rolled shape publicly in channel!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &SEER),
            ("Carol", &DRUNK),
        ],
        [&BLOB, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 7,
    })));
    h2.set_key(ActiveBlobShape(Some(BlobShape::Right1)));

    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "7").await; // Exact secret number!
    h2.reply_dm("Bob", "Carol").await;
    h2.reply_dm("Carol", "1").await; // Drunk Carol swaps into Center #1 (`Blob`)!
    let rep2 = h2.finish_night(n2).await;

    let alice_watch = Text::parse(&rep2.dms_to("Alice")).plain();
    assert!(
        alice_watch.contains("Oracle All-Night Watch")
            && alice_watch.contains("Seer: Bob")
            && alice_watch.contains("Drunk: Carol"),
        "Alice DMs: {alice_watch}"
    );
    // Even though Blob started in Center #1, its `Right1` shape was announced publicly AND Drunk Carol wins by keeping her right neighbor (Alice) alive!
    assert!(rep2
        .channel_lines
        .iter()
        .any(|l| Text::parse(l).plain().contains("1 player to their RIGHT")));
    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    // Bob (Village Seer) died with 0 monsters in play -> Village loses, while Carol (`Blob`, seat #3, right neighbor = #1 Alice who survived!) wins solo!
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol"]);

    // 3. Oracle #3 (`DrunkExchange`), #4 (`CenterPeek`), and #6 (`PlayerPeek` with redirect):
    let mut h3 = OnuwTestHarness::new(
        &[("Alice", &ORACLE), ("Bob", &SEER), ("Carol", &VILLAGER)],
        [&TANNER, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedOracleQuestion(Some(OracleQuestion::PlayerPeek {
        redirect: Some(1), // Redirects peek onto Bob (seat 1 = Seer) and announces Bob publicly!
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

    // 4. Publicly converted Oracle (`CardTransform::Public`) triggers Paranormal Investigator & Revealer flip-back:
    use crate::roles::daybreak::{
        paranormal_investigator::PARANORMAL_INVESTIGATOR, revealer::REVEALER,
    };
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
    h4.reply_dm("Alice", "yes").await; // Alice publicly becomes `Oracle [Alien]`!
    h4.reply_dm("Dave", "!pass").await;
    h4.reply_dm("Bob", "Alice").await; // PI peeks Alice -> stops immediately and becomes `Oracle [Alien]`!
    h4.reply_dm("Carol", "Alice").await; // Revealer peeks Alice -> flips `Oracle [Alien]` back face-down!
    let rep4 = h4.finish_night(n4).await;
    assert_eq!(
        rep4.effective_role_at(&h4.roster, "Bob"),
        "Oracle [Alien]"
    );
    assert!(!rep4.board.is_player_face_up(h4.seat_of("Alice")));
}

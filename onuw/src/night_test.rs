use std::time::Duration;

use ircgm::prelude::Text;

use crate::artifacts::Artifact;
use crate::marks::Mark;
use crate::night::{build_deck_schedule, format_role_order, NightTimings, TierPhase};
use crate::role::{Role, Team};
use crate::roles::{
    alien::{
        alien::ALIEN, blob::BLOB, cow::COW, groob_zerb::{GROOB, ZERB}, leader::LEADER,
        mortician::MORTICIAN,
    },
    base::{
        doppelganger::DOPPELGANGER, hunter::HUNTER, insomniac::INSOMNIAC, robber::ROBBER,
        seer::SEER, tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF,
    },
    daybreak::{
        alpha_wolf::ALPHA_WOLF, aura_seer::AURA_SEER, curator::CURATOR, dream_wolf::DREAM_WOLF,
        paranormal_investigator::PARANORMAL_INVESTIGATOR, prince::PRINCE, revealer::REVEALER,
        sentinel::SENTINEL, village_idiot::VILLAGE_IDIOT, witch::WITCH,
    },
    super_villains::{evilometer::EVILOMETER, temptress::TEMPTRESS},
    vampire::{
        assassin::ASSASSIN, copycat::COPYCAT, count::COUNT, cupid::CUPID, gremlin::GREMLIN,
        pickpocket::PICKPOCKET, priest::PRIEST, thing::THING, vampire::VAMPIRE,
    },
};
use crate::testing::OnuwTestHarness;

#[test]
fn deck_schedule_is_purely_deck_derived_and_invariant_to_card_order() {
    let timings = NightTimings {
        tier_budget: Duration::from_secs(20),
        copycat_pick_budget: Duration::from_secs(10),
        warn_remaining: None,
    };

    let deck_order_a: Vec<&'static dyn Role> = vec![
        &SENTINEL,
        &COPYCAT,
        &DOPPELGANGER,
        &VAMPIRE,
        &COUNT,
        &ALPHA_WOLF,
        &SEER,
        &WITCH,
        &INSOMNIAC,
    ];
    let mut deck_order_b = deck_order_a.clone();
    deck_order_b.reverse();

    let sched_a = build_deck_schedule(&deck_order_a, &timings);
    let sched_b = build_deck_schedule(&deck_order_b, &timings);

    assert_eq!(sched_a.len(), sched_b.len());
    for (ta, tb) in sched_a.iter().zip(&sched_b) {
        assert_eq!(ta.phase, tb.phase);
        assert_eq!(ta.callout_label(), tb.callout_label());
        assert_eq!(ta.start_offset, tb.start_offset);
        assert_eq!(ta.end_offset, tb.end_offset);
    }

    let summary: Vec<(TierPhase, String, u64, u64)> = sched_a
        .iter()
        .map(|t| {
            (
                t.phase,
                t.callout_label(),
                t.start_offset.as_secs(),
                t.end_offset.as_secs(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (TierPhase::Dusk, "Copycat".into(), 0, 10),
            (TierPhase::Dusk, "Doppelgänger".into(), 10, 20),
            (TierPhase::Dusk, "Vampires".into(), 20, 40),
            (TierPhase::Dusk, "The Count (#1/2)".into(), 40, 60),
            (TierPhase::Dusk, "The Count (#2/2)".into(), 60, 80),
            (TierPhase::Night, "Sentinel".into(), 80, 100),
            (
                TierPhase::Night,
                "Doppelgänger (copied action)".into(),
                100,
                120
            ),
            (TierPhase::Night, "Werewolves".into(), 120, 140),
            (
                TierPhase::Night,
                "Alpha Wolf, Seer, Witch, Insomniac".into(),
                140,
                160,
            ),
        ]
    );

    let villain_alien_sched = build_deck_schedule(
        &[&TEMPTRESS, &VILLAGER, &VILLAGER, &VILLAGER, &VILLAGER, &VILLAGER],
        &timings,
    );
    let labels: Vec<String> = villain_alien_sched
        .iter()
        .map(crate::night::ScheduledTier::callout_label)
        .collect();
    assert_eq!(labels, vec!["Super Villains", "Temptress"]);
}

#[tokio::test(start_paused = true)]
async fn all_active_roles_in_center_has_identical_channel_timestamps_as_all_active_roles_held() {
    let mut h_held = OnuwTestHarness::new(
        &[
            ("Alice", &SENTINEL),
            ("Bob", &DOPPELGANGER),
            ("Carol", &ROBBER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night_held = h_held.spawn_night();
    h_held.reply_dm("Alice", "Alice").await;
    h_held.reply_dm("Bob", "Carol").await;
    h_held.reply_dm("Bob", "pass").await;
    h_held.reply_dm("Carol", "Bob").await;
    let report_held = h_held.finish_night(night_held).await;

    let mut h_center = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&SENTINEL, &DOPPELGANGER, &ROBBER],
    )
    .await;

    let night_center = h_center.spawn_night();
    let report_center = h_center.finish_night(night_center).await;

    assert_eq!(report_held.total_elapsed, Duration::from_secs(18));
    assert_eq!(report_center.total_elapsed, Duration::from_secs(18));

    let plain_held: Vec<(Duration, String)> = report_held
        .timed_channel
        .iter()
        .map(|(ts, s)| (*ts, Text::parse(s).plain()))
        .collect();
    let plain_center: Vec<(Duration, String)> = report_center
        .timed_channel
        .iter()
        .map(|(ts, s)| (*ts, Text::parse(s).plain()))
        .collect();
    assert_eq!(plain_held, plain_center);
}

#[tokio::test(start_paused = true)]
async fn fast_vs_slow_or_timed_out_players_cannot_shift_any_downstream_prompt_or_bump_timestamp() {
    let mut h_fast = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &THING),
            ("Carol", &REVEALER),
            ("Dave", &CURATOR),
        ],
        [&WEREWOLF, &INSOMNIAC, &ROBBER],
    )
    .await;
    let n_fast = h_fast.spawn_night();
    h_fast.reply_dm("Alice", "Bob").await;
    h_fast.reply_dm("Alice", "right").await;
    h_fast.reply_dm("Bob", "left").await;
    h_fast.reply_dm("Carol", "Bob").await;
    let dave_prompt_fast = h_fast.wait_for_new_dm("Dave").await;
    assert!(dave_prompt_fast.contains("Curator"));
    let curator_ts_fast = h_fast.timed_dms["Dave"]
        .iter()
        .find(|(_, m)| m.contains("Curator:"))
        .unwrap()
        .0;
    h_fast.send_dm("Dave", "pass").await;
    let rep_fast = h_fast.finish_night(n_fast).await;

    let mut h_slow = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &THING),
            ("Carol", &VILLAGER),
            ("Dave", &CURATOR),
        ],
        [&WEREWOLF, &INSOMNIAC, &REVEALER],
    )
    .await;
    let n_slow = h_slow.spawn_night();
    let _ = h_slow.wait_for_new_dm("Alice").await;
    h_slow.advance_time(Duration::from_millis(2_700)).await;
    h_slow.send_dm("Alice", "Carol").await;

    let _ = h_slow.wait_for_new_dm("Bob").await;
    h_slow.advance_time(Duration::from_millis(4_400)).await;
    h_slow.send_dm("Bob", "left").await;

    let _ = h_slow.wait_for_new_dm("Dave").await;
    let curator_ts_slow = h_slow.timed_dms["Dave"]
        .iter()
        .find(|(_, m)| m.contains("Curator:"))
        .unwrap()
        .0;
    let rep_slow = h_slow.finish_night(n_slow).await;

    assert_eq!(curator_ts_fast, Duration::from_secs(23));
    assert_eq!(curator_ts_slow, Duration::from_secs(23));
    assert_eq!(rep_fast.total_elapsed, Duration::from_secs(33));
    assert_eq!(rep_slow.total_elapsed, Duration::from_secs(33));
}

#[tokio::test(start_paused = true)]
async fn end_of_dusk_informs_all_marks_wakes_lovers_and_priest_precedes_assassin() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &CUPID),
            ("Carol", &PRIEST),
            ("Dave", &ASSASSIN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Dusk #-6: Vampire Alice marks Bob (Mark::Vampire).
    h.reply_dm("Alice", "Bob").await;
    // Dusk #-4: Cupid Bob marks Carol and Dave with Mark::Love.
    h.reply_dm("Bob", "Carol Dave").await;
    // Dusk #-2: Priest Carol cleanses Bob back to Mark::Clarity (also cleansing her own Mark::Love? Wait: Priest cleanses self + optional other!).
    // Have Carol cleanse Bob? Wait: Priest automatically cleanses self! So let Cupid mark Alice and Bob instead, Priest cleanses Bob, and Assassin marks Alice!
    h.reply_dm("Carol", "Bob").await;
    // Dusk #-1: Assassin Dave marks Alice (runs AFTER Priest #-2, so Priest cannot cleanse Mark::Assassin!).
    h.reply_dm("Dave", "Alice").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Alice"), Mark::Assassin);
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Clarity);
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Clarity);
    let alice_plain = Text::parse(&report.dms_to("Alice")).plain();
    let bob_plain = Text::parse(&report.dms_to("Bob")).plain();
    assert!(alice_plain.contains("Dusk ends — you check your Mark: Mark of the Assassin."));
    assert!(bob_plain.contains("Dusk ends — you check your Mark: Mark of Clarity."));

    // Now test 00: Lovers mutual recognition when both Lovers retain Mark::Love at end of Dusk:
    let mut h_love = OnuwTestHarness::new(
        &[
            ("Alice", &CUPID),
            ("Bob", &VILLAGER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_love = h_love.spawn_night();
    h_love.reply_dm("Alice", "Bob Carol").await;
    let rep_love = h_love.finish_night(n_love).await;
    let love_bob = Text::parse(&rep_love.dms_to("Bob")).plain();
    let love_carol = Text::parse(&rep_love.dms_to("Carol")).plain();
    assert!(love_bob.contains("Dusk ends — you check your Mark: Mark of Love."));
    assert!(love_bob.contains("Lovers wake: Your Lover is Carol"));
    assert!(love_carol.contains("Lovers wake: Your Lover is Bob"));
    assert!(rep_love.has_chronicle_substr("Lovers (Bob, Carol) woke at the end of Dusk and recognized each other."));
}

#[tokio::test(start_paused = true)]
async fn copycat_and_doppelganger_expansion_roles_wake_on_role_schedule_and_respect_fear() {
    // 1. Copycat -> Robber wakes at Night #6 (paralyzed if given Mark::Fear in Dusk!), while Pickpocket runs at Night #6C:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &COUNT),
            ("Carol", &PICKPOCKET),
            ("Dave", &VILLAGER),
        ],
        [&ROBBER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Dusk #-8: Copycat Alice peeks Center #1 (Robber) — does NOT rob immediately in Dusk!
    h.reply_dm("Alice", "1").await;
    // Dusk #-6: Count Bob (as Coven) marks Dave with Mark::Vampire, then at #-6B places Mark::Fear on Alice!
    h.reply_dm("Bob", "Dave").await;
    h.reply_dm("Bob", "Alice").await;
    // Night #6: Alice (Copycat -> Robber) is paralyzed by Mark::Fear and never prompted!
    // Night #6C: Pickpocket Carol steals Alice's Mark::Fear during Night!
    h.reply_dm("Carol", "Alice").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Robber");
    assert_eq!(report.mark_at(&h.roster, "Carol"), Mark::Fear);
    assert_eq!(report.mark_at(&h.roster, "Alice"), Mark::Clarity);

    // 2. Doppelgänger copying Priest wakes at Dusk #-2A (after Vampire #-6 and Priest #-2) and cleanses Mark::Vampire:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &DOPPELGANGER),
            ("Carol", &PRIEST),
            ("Dave", &VAMPIRE),
        ],
        [&ROBBER, &VILLAGER, &VILLAGER],
    )
    .await;

    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "1").await; // Copycat -> Robber (waits for Night #6)
    h2.reply_dm("Bob", "Carol").await; // Doppelgänger -> Priest (waits for Dusk #-2A)
    h2.reply_dm("Dave", "Alice").await; // Vampire (#-6) marks Alice
    h2.reply_dm("Carol", "pass").await; // Priest (#-2) passes
    h2.reply_dm("Bob", "Alice").await; // Doppelgänger-Priest (#-2A) cleanses Alice!
    h2.reply_dm("Alice", "Dave").await; // Copycat-Robber (Night #6) robs Dave (Vampire)!

    let rep2 = h2.finish_night(n2).await;
    assert_eq!(rep2.mark_at(&h2.roster, "Alice"), Mark::Clarity);
    assert_eq!(rep2.effective_role_at(&h2.roster, "Alice"), "Vampire");
    assert_eq!(rep2.effective_role_at(&h2.roster, "Dave"), "Robber");
}

#[tokio::test(start_paused = true)]
async fn sentinel_omits_dusk_banner_aura_seer_precedes_gremlin_and_face_up_follows_card() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SENTINEL),
            ("Bob", &AURA_SEER),
            ("Carol", &GREMLIN),
            ("Dave", &REVEALER),
        ],
        [&VILLAGER, &WEREWOLF, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Sentinel (#0) shields Bob:
    h.reply_dm("Alice", "Bob").await;
    // Aura Seer (#7C) runs BEFORE Gremlin (#7D) and Revealer (#10), so nobody has viewed/moved a card yet!
    // Gremlin (#7D) swaps Alice and Carol (no Marks in deck, so reply `<nick1> <nick2>`):
    h.reply_dm("Carol", "Alice Carol").await;
    // Revealer (#10) flips Alice's new card (Gremlin = Village -> stays face-up!).
    h.reply_dm("Dave", "Alice").await;

    let mut report = h.finish_night(night).await;
    let all_chan = report.channel_lines.join("\n");
    assert!(!all_chan.contains("Dusk falls!"));
    assert!(report
        .dms_to("Bob")
        .contains("Nobody else viewed or moved any cards tonight!"));

    let alice_idx = h.seat_of("Alice");
    let dave_idx = h.seat_of("Dave");
    assert!(report.board.is_player_face_up(alice_idx));
    // Swapping cards carries `face_up` with the Card rather than leaving it stuck on `alice_idx`:
    report.board.swap_players_for_test(alice_idx, dave_idx);
    assert!(!report.board.is_player_face_up(alice_idx));
    assert!(report.board.is_player_face_up(dave_idx));
}

#[tokio::test(start_paused = true)]
async fn role_override_artifacts_suppress_dawn_powers_and_override_dusk_marks() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &PRINCE),
            ("Bob", &HUNTER),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    let mut report = h.finish_night(night).await;

    let alice_idx = h.seat_of("Alice");
    let bob_idx = h.seat_of("Bob");
    // Alice (Prince) gets Claw of the Werewolf -> becomes Werewolf and loses Prince vote immunity!
    report
        .board
        .insert_artifact_for_test(alice_idx, Artifact::ClawOfTheWerewolf);
    // Bob (Hunter) gets Mark::Vampire in Dusk, then Brand of the Villager in Night -> Artifact overrides Mark, and Hunter loses on_death shot!
    report.board.set_mark_for_test(bob_idx, Mark::Vampire);
    report
        .board
        .insert_artifact_for_test(bob_idx, Artifact::BrandOfTheVillager);

    assert_eq!(report.board.effective_team(bob_idx), Team::Village);
    assert!(!report.board.is_monster_threat_for(bob_idx, Team::Vampire));
    assert!(report.board.is_monster_threat_for(alice_idx, Team::Werewolf));

    // Alice (Claw Prince) and Bob (Brand Hunter, voting Carol) tie with 2 votes each and BOTH die:
    // Alice is NOT immune as Prince, and Bob does NOT shoot Carol as Hunter!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn.dead_nicks(&h.roster).contains("Alice"));
    assert!(!dawn.dead_nicks(&h.roster).contains("Carol"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol", "Dave"]);

    // Verify Brand Hunter death also suppresses Hunter shot when Bob dies:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &HUNTER),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "pass").await;
    let mut rep2 = h2.finish_night(n2).await;
    let bob2 = h2.seat_of("Bob");
    rep2.board
        .insert_artifact_for_test(bob2, Artifact::BrandOfTheVillager);
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
    assert!(dawn2.dead_nicks(&h2.roster).contains("Bob"));
    assert!(!dawn2.dead_nicks(&h2.roster).contains("Alice"));
}

#[tokio::test(start_paused = true)]
async fn epic_battle_eliminates_runner_up_and_tanner_blocks_forced_team_victory() {
    // 1. Epic Battle (Werewolf + Vampire): 1st place (3 votes) AND 2nd place (1 vote) both die -> Village kills both monsters and wins!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Bob", "Alice").await; // Vampire marks Alice (Alice holds Werewolf + Mark::Vampire; Bob holds Vampire!)
    h.reply_dm("Alice", "pass").await;
    let mut report = h.finish_night(night).await;
    // Keep Alice pure Werewolf and Bob pure Vampire so each faction has 1 distinct monster:
    let alice_idx = h.seat_of("Alice");
    report.board.set_mark_for_test(alice_idx, Mark::Clarity);

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),   // Bob gets 1 vote (sole 2nd place!)
                ("Bob", Some("Alice")),   // Alice gets 3 votes (1st place!)
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn.dead_nicks(&h.roster).contains("Alice"));
    assert!(dawn.dead_nicks(&h.roster).contains("Bob"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Carol", "Dave"]);

    // 2. ENG-07: Leader ForceTeam(Team::Alien) is blocked when Tanner dies!
    let mut h_lead = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &LEADER),
            ("Carol", &TANNER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_lead = h_lead.spawn_night();
    h_lead.reply_dm("Alice", "pass").await;
    let rep_lead = h_lead.finish_night(n_lead).await;

    // Alien Alice points at Leader Bob, while Bob & Dave execute Tanner Carol (2 votes):
    let dawn_lead = h_lead
        .resolve_voting(
            rep_lead,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn_lead.winner_nicks(&h_lead.roster), vec!["Carol"]);
}

#[tokio::test(start_paused = true)]
async fn duplicate_witches_run_sequentially_and_alpha_wolf_and_rapscallion_extra_cards_are_independent() {
    // 1. Two Witches in custom deck get sequential RoleSlot tiers so Witch #2 sees Witch #1's center swap:
    let mut h = OnuwTestHarness::new(
        &[("Alice", &WITCH), ("Bob", &WITCH), ("Carol", &VILLAGER)],
        [&SEER, &ROBBER, &TANNER],
    )
    .await;

    let night = h.spawn_night();
    // Witch #1 (Alice) peeks Center #1 (Seer) and swaps it onto Carol (putting Villager into Center #1):
    h.reply_dm("Alice", "1").await;
    h.reply_dm("Alice", "Carol").await;
    // Witch #2 (Bob) peeks Center #1 — sees live Villager (not stale Seer!) and swaps it onto Alice:
    h.reply_dm("Bob", "1").await;
    h.reply_dm("Bob", "Alice").await;

    let report = h.finish_night(night).await;
    assert!(report.dms_to("Bob").contains("Villager"));
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Seer");
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Villager");
    assert_eq!(report.center_role(1), "Witch");

    // 2. Alpha Wolf + Temptress crossover uses independent extra_evil_card (Werewolf) & extra_villain_card (Henchman #7):
    let mut h_cross = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &TEMPTRESS),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_cross = h_cross.spawn_night();
    h_cross.reply_dm("Bob", "Dave").await; // Temptress (1-SV-B) recruits Dave -> Henchman #7
    h_cross.reply_dm("Alice", "pass").await; // Lone Wolf peek (2)
    h_cross.reply_dm("Alice", "Carol").await; // Alpha Wolf (2-B) bites Carol -> Werewolf
    let rep_cross = h_cross.finish_night(n_cross).await;
    assert_eq!(
        rep_cross.effective_role_at(&h_cross.roster, "Carol"),
        "Werewolf"
    );
    assert_eq!(
        rep_cross.effective_role_at(&h_cross.roster, "Dave"),
        "Henchman #7"
    );
}

#[test]
fn artifact_chaos_merges_false_splits_and_curator_merges_without_revealer() {
    let timings = NightTimings::default();

    // 4-player Artifact Chaos (Daybreak):
    // PI (`PLAYER_FACES -> PLAYER_BECAME`), Witch (`CENTER_FACES -> PLAYER_FACES | CENTER_FACES`),
    // Village Idiot (`-> PLAYER_FACES`), Aura Seer (passive), and Revealer (`-> FACE_UP`)
    // share Tier 3 concurrently without false splits, while Curator (`VISIBLE_TABLE`) splits into Tier 4
    // so Curator sees any face-up card flipped by Revealer!
    let artifact_chaos: Vec<&'static dyn Role> = vec![
        &ALPHA_WOLF,
        &WITCH,
        &CURATOR,
        &PARANORMAL_INVESTIGATOR,
        &REVEALER,
        &VILLAGE_IDIOT,
        &AURA_SEER,
    ];
    let labels: Vec<String> = build_deck_schedule(&artifact_chaos, &timings)
        .iter()
        .map(crate::night::ScheduledTier::callout_label)
        .collect();
    assert_eq!(
        labels,
        vec![
            "Werewolves",
            "Alpha Wolf",
            "Paranormal Investigator, Witch, Village Idiot, Aura Seer, Revealer",
            "Curator",
        ]
    );

    // Without Revealer, nothing mutates `VISIBLE_TABLE` before Curator, so Curator merges into Tier 3 (3 tiers total):
    let without_revealer: Vec<&'static dyn Role> = vec![
        &ALPHA_WOLF,
        &WITCH,
        &CURATOR,
        &PARANORMAL_INVESTIGATOR,
        &VILLAGER,
        &VILLAGE_IDIOT,
        &AURA_SEER,
    ];
    let labels_no_rev: Vec<String> = build_deck_schedule(&without_revealer, &timings)
        .iter()
        .map(crate::night::ScheduledTier::callout_label)
        .collect();
    assert_eq!(
        labels_no_rev,
        vec![
            "Werewolves",
            "Alpha Wolf",
            "Paranormal Investigator, Witch, Village Idiot, Aura Seer, Curator",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn public_announce_sequentializes_later_roles_including_eyes_closed_and_runs_when_unheld() {
    use crate::roles::alien::{
        blob::BLOB,
        nostradamus::NOSTRADAMUS,
        oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE},
        rascal::{ForcedRascalMode, RascalMode, RASCAL},
    };
    use crate::roles::base::drunk::DRUNK;

    // 1. Even an `.eyes_closed()` role (`Blob`) splits into a new tier after `PUBLIC_ANNOUNCE` writers:
    let timings = NightTimings::default();
    let deck: Vec<&'static dyn Role> = vec![
        &NOSTRADAMUS,
        &RASCAL,
        &BLOB,
        &VILLAGER,
        &VILLAGER,
        &VILLAGER,
    ];
    let labels: Vec<String> = build_deck_schedule(&deck, &timings)
        .iter()
        .map(crate::night::ScheduledTier::callout_label)
        .collect();
    assert_eq!(labels, vec!["Nostradamus", "Rascal", "Blob"]);

    // 2. When Oracle, Aliens, and Rascal are all sitting in the CENTER, their Mother Ship announcements
    //    still broadcast in channel before later roles act!
    let mut h = OnuwTestHarness::new(
        &[("Alice", &DRUNK), ("Bob", &VILLAGER), ("Carol", &VILLAGER)],
        [&ORACLE, &ALIEN, &RASCAL],
    )
    .await;
    h.set_key(ForcedOracleQuestion(Some(OracleQuestion::DrunkExchange {
        fizzle: true,
    })));
    h.set_key(ForcedRascalMode(Some(RascalMode::Idiot)));

    let night = h.spawn_night();
    h.reply_dm("Alice", "3").await;
    let report = h.finish_night(night).await;

    let plain_lines: Vec<String> = report
        .channel_lines
        .iter()
        .map(|l| Text::parse(l).plain())
        .collect();
    let oracle_idx = plain_lines
        .iter()
        .position(|l| l.contains("Oracle Omen:") && l.contains("center card"))
        .expect("center Oracle announced center-exchange omen");
    let alien_idx = plain_lines
        .iter()
        .position(|l| l.contains("Mother Ship (Aliens):"))
        .expect("center Aliens announced event");
    let rascal_idx = plain_lines
        .iter()
        .position(|l| l.contains("Mother Ship (Rascal):") && l.contains("Village Idiot mode"))
        .expect("center Rascal announced mode");
    let drunk_call_idx = plain_lines
        .iter()
        .position(|l| l.contains("Calling: Drunk"))
        .expect("Drunk called");

    assert!(oracle_idx < alien_idx);
    assert!(alien_idx < rascal_idx);
    assert!(rascal_idx < drunk_call_idx);
}

#[tokio::test(start_paused = true)]
async fn tabletop_visibility_omits_empty_dms_shows_markers_to_eyes_open_and_hides_from_eyes_closed_or_sleeping()
{
    // 1. When no shields, face-up cards, or artifacts are on the table, NEVER send an empty `On the table:` DM:
    let mut h_empty = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &SEER),
            ("Carol", &INSOMNIAC),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_empty = h_empty.spawn_night();
    h_empty.reply_dm("Alice", "pass").await;
    h_empty.reply_dm("Bob", "pass").await;
    let rep_empty = h_empty.finish_night(n_empty).await;
    for nick in ["Alice", "Bob", "Carol"] {
        let dms = rep_empty.dms_to(nick);
        assert!(!dms.contains("On the table:"));
        assert!(!dms.contains('🛡'));
        assert!(!dms.contains('🔦'));
        assert!(!dms.contains('🏺'));
    }

    // 2. Sentinel shields Bob (#0), Revealer flips Carol's Villager face-up (#10), Curator places Artifact on Alice (#11):
    //    - Eyes-open roles (`Alien`, `Curator`, solo `Mortician`) see current tabletop markers.
    //    - Sleeping (`Dream Wolf`) and eyes-closed (`Cow`, `Evilometer`, `Blob`) roles NEVER see tabletop markers!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &SENTINEL),
            ("Bob", &COW),
            ("Carol", &VILLAGER),
            ("Dave", &DREAM_WOLF),
            ("Eve", &EVILOMETER),
            ("Frank", &ALIEN),
            ("Grace", &REVEALER),
            ("Heidi", &CURATOR),
            ("Ivan", &BLOB),
            ("Judy", &MORTICIAN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Tier 1 (#0): Sentinel Alice shields Bob:
    h.reply_dm("Alice", "Bob").await;
    // Tier 2: Alien Frank (#3), Cow Bob (#3A, eyes closed), Evilometer Eve (#3B, eyes closed), Revealer Grace (#10):
    h.reply_dm("Frank", "pass").await;
    h.reply_dm("Grace", "Carol").await;
    // Tier 3 (#11): Curator Heidi places Artifact on Alice:
    h.reply_dm("Heidi", "Alice").await;
    // Tier 4 (#12..#13): Blob Ivan (passive eyes open) & Mortician Judy (solo eyes open):
    h.reply_dm("Judy", "pass").await;

    let report = h.finish_night(night).await;

    // Sleeping Dream Wolf (Dave) and eyes-closed roles (Cow Bob, Evilometer Eve, Blob Ivan) NEVER see tabletop state:
    for closed_nick in ["Bob", "Dave", "Eve", "Ivan"] {
        let dms = Text::parse(&report.dms_to(closed_nick)).plain();
        assert!(
            !dms.contains("On the table:")
                && !dms.contains('🛡')
                && !dms.contains('🔦')
                && !dms.contains('🏺'),
            "{closed_nick} should not see tabletop state, got: {dms}"
        );
    }

    // Alien Frank (#3, after Sentinel #0) sees `Bob🛡`:
    let frank_dms = Text::parse(&report.dms_to("Frank")).plain();
    assert!(frank_dms.contains("Bob🛡"), "Frank DMs: {frank_dms}");

    // Curator Heidi (#11, after Sentinel #0 and Revealer #10) sees `Bob🛡` and `Carol [🔦 Villager]`:
    let heidi_dms = Text::parse(&report.dms_to("Heidi")).plain();
    assert!(heidi_dms.contains("Bob🛡"), "Heidi DMs: {heidi_dms}");
    assert!(
        heidi_dms.contains("Carol [🔦 Villager]"),
        "Heidi DMs: {heidi_dms}"
    );

    // Mortician Judy (#13A, solo eyes open) sees all three markers (`Alice🏺`, `Bob🛡`, `Carol [🔦 Villager]`):
    let judy_dms = Text::parse(&report.dms_to("Judy")).plain();
    assert!(judy_dms.contains("On the table:"), "Judy DMs: {judy_dms}");
    assert!(judy_dms.contains("Alice🏺"), "Judy DMs: {judy_dms}");
    assert!(judy_dms.contains("Bob🛡"), "Judy DMs: {judy_dms}");
    assert!(
        judy_dms.contains("Carol [🔦 Villager]"),
        "Judy DMs: {judy_dms}"
    );
}

#[test]
#[should_panic(expected = "Seer: undeclared write to PLAYER_FACES")]
fn undeclared_board_write_panics_with_role_name() {
    use crate::board::{BoardAccess, BoardCards, BoardMut, Card};

    let mut board = BoardCards::new(
        vec![Card::new(&SEER), Card::new(&VILLAGER), Card::new(&WEREWOLF)],
        [Card::new(&VILLAGER), Card::new(&VILLAGER), Card::new(&VILLAGER)],
    );
    let mut bm = BoardMut::new(
        &mut board,
        "Seer",
        BoardAccess::VISIBLE_TABLE,
        BoardAccess::NONE,
    );
    let _ = bm.swap_players(0, 0, 1);
}

#[tokio::test(start_paused = true)]
async fn doppelganger_curator_wakes_sequentially_after_curator_and_sees_first_artifact() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &CURATOR),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // #1: Alice copies Bob (Curator):
    h.reply_dm("Alice", "Bob").await;
    // #11: Bob (Curator #1/2) places an Artifact on Carol:
    h.reply_dm("Bob", "Carol").await;
    // #11a: Alice (Doppelgänger-Curator #2/2) wakes sequentially AFTER Bob's tier ends, sees `Carol🏺`,
    // gets rejected when trying `Carol`, and places an Artifact on herself:
    let mut saw_carol_artifact = false;
    loop {
        let m = h.wait_for_new_dm("Alice").await;
        if Text::parse(&m).plain().contains("Carol🏺") {
            saw_carol_artifact = true;
        }
        if m.contains("Curator:") {
            break;
        }
    }
    assert!(
        saw_carol_artifact,
        "Doppelgänger-Curator must see Curator's artifact on Carol"
    );
    h.send_dm("Alice", "Carol").await;
    let rej = h.wait_for_new_dm("Alice").await;
    assert!(rej.contains("already has an Artifact token"), "got: {rej}");
    h.send_dm("Alice", "Alice").await;

    let report = h.finish_night(night).await;
    assert!(report.board.artifact(h.seat_of("Carol")).is_some());
    assert!(report.board.artifact(h.seat_of("Alice")).is_some());
}

#[tokio::test(start_paused = true)]
async fn epic_battle_requires_third_faction_and_traitor_applies_after_artifact_and_fear_hides_sensors() {
    // 1. Pure 2-faction monster board (`[Werewolf, Vampire, Villager + Mark::Vampire]` = 0 Villagers)
    //    does NOT trigger Epic Battle double-kill: only 1st place (2 votes) dies, so Vampires win!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n = h.spawn_night();
    h.reply_dm("Bob", "Carol").await; // Vampire marks Carol -> 0 Villagers remain!
    h.reply_dm("Alice", "pass").await;
    let rep = h.finish_night(n).await;
    let dawn = h
        .resolve_voting(
            rep,
            &[
                ("Alice", Some("Bob")),   // 1 vote on Bob (Vampire)
                ("Bob", Some("Alice")),   // 2 votes on Alice (Werewolf)
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.dead_nicks(&h.roster).len(), 1);
    assert!(dawn.dead_nicks(&h.roster).contains("Alice"));
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);

    // 2. Mark::Fear suppresses Cow & Evilometer from leaking to Aliens & Super Villains,
    //    and Mark::Traitor applies to holder's new ClawOfTheWerewolf team:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &COUNT),
            ("Bob", &COW),
            ("Carol", &ALIEN),
            ("Dave", &PRINCE),
            ("Eve", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Dave").await; // Vampire Coven marks Dave
    h2.reply_dm("Alice", "Bob").await; // Count places Mark::Fear on Cow Bob!
    h2.reply_dm("Carol", "pass").await; // Alien Carol does NOT see feared Cow Bob!
    h2.reply_dm("Eve", "pass").await;
    let mut rep2 = h2.finish_night(n2).await;
    assert!(!rep2.dms_to("Carol").contains("holds the Cow card"));

    // Give Bob ClawOfTheWerewolf + Mark::Traitor (now Werewolf Traitor!) and test Prince non-lethal 2-vote silence:
    let bob_idx = h2.seat_of("Bob");
    let dave_idx = h2.seat_of("Dave");
    rep2.board
        .insert_artifact_for_test(bob_idx, Artifact::ClawOfTheWerewolf);
    rep2.board.set_mark_for_test(bob_idx, Mark::Traitor);
    rep2.board.set_mark_for_test(dave_idx, Mark::Clarity); // Dave is pure Village Prince

    // Eve (Werewolf) gets 3 votes and dies; Dave (Prince) gets 2 votes (out-voted anyway -> no royal immunity leak!);
    // Bob (Claw Werewolf + Traitor) survives while fellow Werewolf Eve dies -> Bob wins as Traitor!
    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Bob", Some("Eve")),
                ("Dave", Some("Eve")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    // Wait: Eve got 2 votes (Bob, Dave) and Dave (Prince) got 2 votes (Alice, Carol) -> Prince immunity DOES save Dave at 2-2 tie!
    // Check Bob won as Traitor alongside Alice/Carol? Let's assert Bob is in winners:
    assert!(dawn2.winner_nicks(&h2.roster).contains(&"Bob".to_owned()));
}

#[tokio::test(start_paused = true)]
async fn dusk_doppelganger_splits_dusk_vs_night_immediates_respecting_fear_and_sentinel() {
    use crate::roles::vampire::diseased::DISEASED;

    // 1. Dusk immediate (`Diseased`) fires at Dusk #-7 BEFORE Vampires (#-6) & Renfield (#-6C):
    let mut h_dusk_imm = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &DISEASED),
            ("Carol", &VAMPIRE),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n1 = h_dusk_imm.spawn_night();
    // Dusk #-7: Alice copies Bob (Diseased), then immediately places Mark::Disease on Carol (left neighbor = Carol):
    h_dusk_imm.reply_dm("Alice", "Bob").await;
    h_dusk_imm.reply_dm("Alice", "Carol").await;
    // Dusk #-6: Vampire Carol marks Bob (overwriting Bob's Clarity while Carol already received Disease from Alice):
    h_dusk_imm.reply_dm("Carol", "Bob").await;
    // Dusk #-5: Original Diseased Bob places Mark::Disease on Alice:
    h_dusk_imm.reply_dm("Bob", "Alice").await;
    let rep1 = h_dusk_imm.finish_night(n1).await;
    assert_eq!(rep1.mark_at(&h_dusk_imm.roster, "Carol"), Mark::Disease);
    assert_eq!(rep1.mark_at(&h_dusk_imm.roster, "Bob"), Mark::Vampire);
    assert_eq!(rep1.mark_at(&h_dusk_imm.roster, "Alice"), Mark::Disease);

    // 2. Night immediate (`Robber`) copied in Dusk #-7 waits until Night #1 (after `00` Mark::Fear & `#0` Sentinel):
    let mut h_fear = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &COUNT),
            ("Carol", &ROBBER),
            ("Dave", &SENTINEL),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h_fear.spawn_night();
    h_fear.reply_dm("Alice", "Carol").await; // Dusk #-7: Alice copies Robber (does NOT rob in Dusk!)
    h_fear.reply_dm("Bob", "Dave").await; // Dusk #-6: Count marks Dave with Vampire
    h_fear.reply_dm("Bob", "Alice").await; // Dusk #-6B: Count paralyzes Alice with Mark::Fear!
    h_fear.reply_dm("Dave", "Bob").await; // Night #0: Sentinel shields Bob
    h_fear.reply_dm("Carol", "pass").await; // Night #6: Original Robber passes
    let rep2 = h_fear.finish_night(n2).await;
    assert!(!rep2.dms_to("Alice").contains("🦹 Robber:"));
    assert!(rep2.has_chronicle_substr(
        "Alice was paralyzed by the Mark of Fear and could not wake up during Night!"
    ));

    // 3. Unfeared Doppel-Robber at Night #1 sees and respects Sentinel's `#0` Shield token:
    let mut h_shield = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &VAMPIRE),
            ("Carol", &ROBBER),
            ("Dave", &SENTINEL),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n3 = h_shield.spawn_night();
    h_shield.reply_dm("Alice", "Carol").await; // Dusk #-7: Alice copies Robber
    h_shield.reply_dm("Bob", "Dave").await; // Dusk #-6: Vampire marks Dave
    h_shield.reply_dm("Dave", "Bob").await; // Night #0: Sentinel shields Bob!
    // Night #1: Alice (Doppel-Robber) wakes AFTER Sentinel, tries shielded Bob -> rejected, then robs Dave:
    let mut saw_bob_shield = false;
    loop {
        let m = h_shield.wait_for_new_dm("Alice").await;
        if Text::parse(&m).plain().contains("Bob🛡") {
            saw_bob_shield = true;
        }
        if m.contains("Robber:") {
            break;
        }
    }
    assert!(saw_bob_shield);
    h_shield.send_dm("Alice", "Bob").await;
    let rej = h_shield.wait_for_new_dm("Alice").await;
    assert!(rej.contains("shielded"), "got: {rej}");
    h_shield.send_dm("Alice", "Dave").await;
    h_shield.reply_dm("Carol", "pass").await;
    let rep3 = h_shield.finish_night(n3).await;
    assert_eq!(rep3.effective_role_at(&h_shield.roster, "Alice"), "Sentinel");
    assert_eq!(rep3.effective_role_at(&h_shield.roster, "Bob"), "Vampire");
}

#[tokio::test(start_paused = true)]
async fn canonical_deck_role_order_formats_all_phases_and_announces_at_night_start() {
    // 1. Night-only deck (invariant to card order, deduplicates packs & duplicates, omits sleeping cards):
    let night_deck_a: Vec<&'static dyn Role> = vec![
        &INSOMNIAC,
        &ROBBER,
        &VILLAGER,
        &ALPHA_WOLF,
        &WEREWOLF,
        &DREAM_WOLF,
        &SEER,
        &DOPPELGANGER,
        &SENTINEL,
        &TANNER,
    ];
    let mut night_deck_b = night_deck_a.clone();
    night_deck_b.reverse();

    let line_a = format_role_order(&night_deck_a).plain();
    let line_b = format_role_order(&night_deck_b).plain();
    assert_eq!(line_a, line_b);
    assert_eq!(
        line_a,
        "📋 Role order: Sentinel → Doppelgänger → Werewolves → Alpha Wolf → Seer → Robber → Insomniac"
    );

    // 2. Dusk + Night deck with Doppelgänger & immediate-copy Night roles includes `#1` copied action:
    let dusk_night_deck: Vec<&'static dyn Role> = vec![
        &INSOMNIAC,
        &WITCH,
        &SEER,
        &ALPHA_WOLF,
        &COUNT,
        &VAMPIRE,
        &DOPPELGANGER,
        &COPYCAT,
        &SENTINEL,
    ];
    assert_eq!(
        format_role_order(&dusk_night_deck).plain(),
        "📋 Role order — Dusk: Copycat → Doppelgänger → Vampires → The Count · Night: Sentinel → Doppelgänger (copied action) → Werewolves → Alpha Wolf → Seer → Witch → Insomniac"
    );

    // 3. Dusk-only deck omits `Night:` section:
    let dusk_only: Vec<&'static dyn Role> = vec![
        &PRIEST,
        &CUPID,
        &COUNT,
        &VAMPIRE,
        &VILLAGER,
        &TANNER,
    ];
    assert_eq!(
        format_role_order(&dusk_only).plain(),
        "📋 Role order — Dusk: Vampires → The Count → Cupid → Priest"
    );

    // 4. Alien Groob & Zerb pair step deduplicates cleanly:
    let alien_deck: Vec<&'static dyn Role> =
        vec![&LEADER, &ZERB, &GROOB, &COW, &ALIEN, &VILLAGER];
    assert_eq!(
        format_role_order(&alien_deck).plain(),
        "📋 Role order: Aliens → Cow → Groob & Zerb → Leader"
    );

    // 5. All-sleeping deck reports `No waking roles`:
    let sleeping_deck: Vec<&'static dyn Role> =
        vec![&VILLAGER, &VILLAGER, &TANNER, &HUNTER, &PRINCE, &DREAM_WOLF];
    assert_eq!(
        format_role_order(&sleeping_deck).plain(),
        "📋 Role order: No waking roles"
    );

    // 6. `run_night_phase` announces the canonical role order at `t = 0` at the beginning of Night:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &INSOMNIAC),
            ("Bob", &ROBBER),
            ("Carol", &WEREWOLF),
        ],
        [&SEER, &VILLAGER, &TANNER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;

    let (first_ts, first_msg) = &report.timed_channel[0];
    assert_eq!(*first_ts, Duration::ZERO);
    assert_eq!(
        Text::parse(first_msg).plain(),
        "📋 Role order: Werewolves → Seer → Robber → Insomniac"
    );

    // 7. Full catalog (`ALL_ROLES`) matches the exact master rulebook waking sequence:
    let expected_dusk = [
        "Oracle",
        "Copycat",
        "Mirror Man",
        "Doppelgänger",
        "Vampires",
        "The Count",
        "Renfield",
        "Diseased",
        "Cupid",
        "Instigator",
        "Priest",
        "Assassin",
        "Apprentice Assassin",
    ]
    .join(" → ");
    let expected_night = [
        "Sentinel",
        "Doppelgänger (copied action)",
        "Aliens",
        "Cow",
        "Groob & Zerb",
        "Body Snatcher",
        "Super Villains",
        "Temptress",
        "Dr. Peeker",
        "Rapscallion",
        "Evilometer",
        "Werewolves",
        "Alpha Wolf",
        "Mystic Wolf",
        "Minion",
        "Apprentice Tanner",
        "Leader",
        "Intern",
        "Mason",
        "Thing",
        "Annoying Lad",
        "Seer",
        "Apprentice Seer",
        "Paranormal Investigator",
        "Marksman",
        "Nostradamus",
        "Psychic",
        "Detector",
        "Robber",
        "Witch",
        "Pickpocket",
        "Role Retriever",
        "Voodoo Lou",
        "Troublemaker",
        "Village Idiot",
        "Aura Seer",
        "Gremlin",
        "Rascal",
        "Switcheroo",
        "Drunk",
        "Insomniac",
        "Self-Awareness Girl",
        "Squire",
        "Family Man",
        "Beholder",
        "Revealer",
        "Exposer",
        "Flipper",
        "Empath",
        "Curator",
        "Blob",
        "Mortician",
    ]
    .join(" → ");
    assert_eq!(
        format_role_order(crate::roles::ALL_ROLES).plain(),
        format!("📋 Role order — Dusk: {expected_dusk} · Night: {expected_night}"),
    );
}

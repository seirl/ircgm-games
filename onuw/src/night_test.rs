use std::time::Duration;

use ircgm::prelude::Text;

use crate::artifacts::Artifact;
use crate::marks::Mark;
use crate::night::{build_deck_schedule, NightTimings, TierPhase};
use crate::role::{Role, Team};
use crate::roles::{
    alien::{alien::ALIEN, leader::LEADER},
    base::{
        doppelganger::DOPPELGANGER, hunter::HUNTER, insomniac::INSOMNIAC, robber::ROBBER,
        seer::SEER, tanner::TANNER, villager::VILLAGER, werewolf::WEREWOLF,
    },
    daybreak::{
        alpha_wolf::ALPHA_WOLF, aura_seer::AURA_SEER, prince::PRINCE, revealer::REVEALER,
        sentinel::SENTINEL, witch::WITCH,
    },
    super_villains::rapscallion::RAPSCALLION,
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
            (
                TierPhase::Dusk,
                "Doppelgänger (copied action)".into(),
                20,
                40
            ),
            (TierPhase::Dusk, "Vampire".into(), 40, 60),
            (TierPhase::Dusk, "Count".into(), 60, 80),
            (TierPhase::Night, "Sentinel".into(), 80, 100),
            (TierPhase::Night, "Werewolf".into(), 100, 120),
            (TierPhase::Night, "Alpha Wolf, Seer".into(), 120, 140),
            (TierPhase::Night, "Witch, Insomniac".into(), 140, 160),
        ]
    );
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
            ("Carol", &ROBBER),
            ("Dave", &WITCH),
        ],
        [&WEREWOLF, &INSOMNIAC, &VILLAGER],
    )
    .await;
    let n_fast = h_fast.spawn_night();
    h_fast.reply_dm("Alice", "Carol").await;
    h_fast.reply_dm("Alice", "Dave").await;
    h_fast.reply_dm("Bob", "left").await;
    h_fast.reply_dm("Carol", "Bob").await;
    let dave_prompt_fast = h_fast.wait_for_new_dm("Dave").await;
    assert!(dave_prompt_fast.contains("Witch"));
    let witch_ts_fast = h_fast.timed_dms["Dave"]
        .iter()
        .find(|(_, m)| m.contains("Witch:"))
        .unwrap()
        .0;
    h_fast.send_dm("Dave", "pass").await;
    let rep_fast = h_fast.finish_night(n_fast).await;

    let mut h_slow = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &THING),
            ("Carol", &VILLAGER),
            ("Dave", &WITCH),
        ],
        [&WEREWOLF, &INSOMNIAC, &ROBBER],
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
    let witch_ts_slow = h_slow.timed_dms["Dave"]
        .iter()
        .find(|(_, m)| m.contains("Witch:"))
        .unwrap()
        .0;
    let rep_slow = h_slow.finish_night(n_slow).await;

    assert_eq!(witch_ts_fast, Duration::from_secs(13));
    assert_eq!(witch_ts_slow, Duration::from_secs(13));
    assert_eq!(rep_fast.total_elapsed, Duration::from_secs(18));
    assert_eq!(rep_slow.total_elapsed, Duration::from_secs(18));
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
    report.board.players.swap(alice_idx, dave_idx);
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
        .artifacts
        .insert(alice_idx, Artifact::ClawOfTheWerewolf);
    // Bob (Hunter) gets Mark::Vampire in Dusk, then Brand of the Villager in Night -> Artifact overrides Mark, and Hunter loses on_death shot!
    report.board.marks[bob_idx] = Mark::Vampire;
    report
        .board
        .artifacts
        .insert(bob_idx, Artifact::BrandOfTheVillager);

    assert_eq!(report.board.effective_team(bob_idx), Team::Village);
    assert!(!report.board.is_vampire_threat(bob_idx));
    assert!(report.board.is_wolf_threat(alice_idx));

    // Alice (Claw Prince) and Bob (Brand Hunter, voting Carol) tie with 2 votes each and BOTH die:
    // Alice is NOT immune as Prince, and Bob does NOT shoot Carol as Hunter!
    let dawn = h
        .resolve_dawn(
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
        .artifacts
        .insert(bob2, Artifact::BrandOfTheVillager);
    let dawn2 = h2
        .resolve_dawn(
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
    report.board.marks[alice_idx] = Mark::Clarity;

    let dawn = h
        .resolve_dawn(
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
        .resolve_dawn(
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

    // 2. Alpha Wolf + Rapscallion crossover uses independent extra_evil_card (Werewolf) & extra_villain_card (Henchman):
    let mut h_cross = OnuwTestHarness::new(
        &[
            ("Alice", &ALPHA_WOLF),
            ("Bob", &RAPSCALLION),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_cross = h_cross.spawn_night();
    h_cross.reply_dm("Alice", "pass").await; // Lone Wolf peek
    h_cross.reply_dm("Alice", "Carol").await; // Alpha Wolf bites Carol -> Werewolf
    h_cross.reply_dm("Bob", "pass").await; // Solo Villain peek
    h_cross.reply_dm("Bob", "Dave").await; // Rapscallion recruits Dave -> Henchman
    let rep_cross = h_cross.finish_night(n_cross).await;
    assert_eq!(
        rep_cross.effective_role_at(&h_cross.roster, "Carol"),
        "Werewolf"
    );
    assert_eq!(
        rep_cross.effective_role_at(&h_cross.roster, "Dave"),
        "Henchman"
    );
}

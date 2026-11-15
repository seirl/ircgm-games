use std::time::Duration;

use ircgm::prelude::Text;

use crate::night::{build_deck_schedule, NightTimings, TierPhase};
use crate::role::Role;
use crate::roles::{
    base::{
        doppelganger::DOPPELGANGER, insomniac::INSOMNIAC, robber::ROBBER, seer::SEER,
        villager::VILLAGER, werewolf::WEREWOLF,
    },
    daybreak::{alpha_wolf::ALPHA_WOLF, sentinel::SENTINEL, witch::WITCH},
    super_villains::rogue::ROGUE,
    vampire::{copycat::COPYCAT, count::COUNT, thing::THING, vampire::VAMPIRE},
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
        &ROGUE,
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
            (TierPhase::Dusk, "Sentinel".into(), 0, 20),
            (TierPhase::Dusk, "Copycat".into(), 20, 30),
            (TierPhase::Dusk, "Copycat (copied action)".into(), 30, 50),
            (TierPhase::Dusk, "Doppelgänger".into(), 50, 60),
            (
                TierPhase::Dusk,
                "Doppelgänger (copied action)".into(),
                60,
                80
            ),
            (TierPhase::Dusk, "Vampire".into(), 80, 100),
            (TierPhase::Dusk, "Count".into(), 100, 120),
            (TierPhase::Night, "Werewolf".into(), 120, 140),
            (TierPhase::Night, "Alpha Wolf, Seer".into(), 140, 160),
            (TierPhase::Night, "Witch".into(), 160, 180),
            (TierPhase::Night, "Rogue, Insomniac".into(), 180, 200),
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
            ("Dave", &ROGUE),
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
    assert!(dave_prompt_fast.contains("Rogue"));
    let rogue_ts_fast = h_fast.timed_dms["Dave"]
        .iter()
        .find(|(_, m)| m.contains("Rogue:"))
        .unwrap()
        .0;
    h_fast.send_dm("Dave", "pass").await;
    let rep_fast = h_fast.finish_night(n_fast).await;

    let mut h_slow = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &THING),
            ("Carol", &VILLAGER),
            ("Dave", &ROGUE),
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
    let rogue_ts_slow = h_slow.timed_dms["Dave"]
        .iter()
        .find(|(_, m)| m.contains("Rogue:"))
        .unwrap()
        .0;
    let rep_slow = h_slow.finish_night(n_slow).await;

    assert_eq!(rogue_ts_fast, Duration::from_secs(13));
    assert_eq!(rogue_ts_slow, Duration::from_secs(13));
    assert_eq!(rep_fast.total_elapsed, Duration::from_secs(18));
    assert_eq!(rep_slow.total_elapsed, Duration::from_secs(18));
}

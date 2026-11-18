use ircgm::prelude::Text;

use crate::roles::{
    alien::{
        alien::{AlienEvent, ForcedAlienEvent, ALIEN, SYNTHETIC_ALIEN},
        groob_zerb::{GROOB, ZERB},
    },
    base::{seer::SEER, tanner::TANNER, villager::VILLAGER},
    daybreak::aura_seer::AURA_SEER,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn alien_bioscan_silent_point_aura_seer_tracking_and_synthetic_alien_solo_win_work() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &SYNTHETIC_ALIEN),
            ("Carol", &SEER),
            ("Dave", &AURA_SEER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::BioScan)));

    let night = h.spawn_night();
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Bob").await;
    h.send_dm("Alice", "blend in, Bob").await;
    let talk_rej = h.wait_for_new_dm("Alice").await;
    assert!(talk_rej.contains("Silent night"), "got: {talk_rej}");
    h.send_dm("Bob", "Eve").await;
    h.send_dm("Alice", "Carol").await;
    h.reply_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert!(!report.dms_to("Bob").contains("blend in, Bob"));
    assert!(report.dms_to("Alice").contains("Bob points at Eve!"));
    assert!(report.dms_to("Bob").contains("Alice points at Carol!"));
    assert!(report.dms_to("Bob").contains("Carol holds Seer"));
    // Both waking Aliens viewed the scanned card -> both recorded for Aura Seer!
    let dave_plain = Text::parse(&report.dms_to("Dave")).plain();
    assert!(dave_plain.contains("Alice, Bob"), "got: {dave_plain}");

    // Synthetic Alien (Bob) dies alongside Alien (Alice) on a 2-2 tie -> Synthetic Alien wins ALONE!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob"]);
}

#[tokio::test(start_paused = true)]
async fn alien_tractor_beam_plurality_matches_reversed_seat_pair_order() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &TANNER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::TractorBeam)));

    let night = h.spawn_night();
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Bob").await;
    h.wait_for_prompt_dm("Carol").await;
    h.send_dm("Bob", "Eve Dave").await;
    h.send_dm("Alice", "Dave Eve").await;
    h.send_dm("Carol", "pass").await;

    let report = h.finish_night(night).await;
    assert_eq!(report.effective_role_at(&h.roster, "Dave"), "Villager");
    assert_eq!(report.effective_role_at(&h.roster, "Eve"), "Tanner");
    // Confirmation DM also reaches actor Bob:
    assert!(report.dms_to("Bob").contains("Tractor Beam swapped"));

    // Passing Aliens in TractorBeam do NOT trigger Aura Seer — only the swapping Alien does!
    let mut h_aura = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &ALIEN),
            ("Carol", &AURA_SEER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_aura.set_key(ForcedAlienEvent(Some(AlienEvent::TractorBeam)));
    let n_aura = h_aura.spawn_night();
    h_aura.wait_for_prompt_dm("Alice").await;
    h_aura.wait_for_prompt_dm("Bob").await;
    h_aura.send_dm("Bob", "pass").await;
    h_aura.send_dm("Alice", "Carol Dave").await;
    let rep_aura = h_aura.finish_night(n_aura).await;
    let carol_plain = Text::parse(&rep_aura.dms_to("Carol")).plain();
    assert!(carol_plain.contains("Alice"));
    assert!(!carol_plain.contains("Bob"));
}

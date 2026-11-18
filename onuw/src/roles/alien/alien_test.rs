use ircgm::prelude::Text;

use crate::roles::{
    alien::{
        alien::{AlienEvent, ForcedAlienEvent, ALIEN, SYNTHETIC_ALIEN},
        groob_zerb::{GROOB, ZERB},
    },
    base::{seer::SEER, villager::VILLAGER},
    daybreak::{aura_seer::AURA_SEER, sentinel::SENTINEL},
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
async fn alien_pass_left_pass_right_and_solo_standard_recon_match_official_directives() {
    // 1. PassRight rotates unshielded waking Aliens one step RIGHT (Alice -> Bob -> Carol -> Alice),
    //    skips Sentinel-shielded Alien Dave, and records ONLY the unshielded rotating Aliens for Aura Seer:
    let mut h_right = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &SYNTHETIC_ALIEN),
            ("Carol", &GROOB),
            ("Dave", &ZERB),
            ("Eve", &SENTINEL),
            ("Frank", &AURA_SEER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_right.set_key(ForcedAlienEvent(Some(AlienEvent::PassRight)));
    let n_right = h_right.spawn_night();
    h_right.reply_dm("Eve", "Dave").await; // Shield Dave's Zerb card!
    let rep_right = h_right.finish_night(n_right).await;

    assert_eq!(rep_right.effective_role_at(&h_right.roster, "Alice"), "Groob");
    assert_eq!(rep_right.effective_role_at(&h_right.roster, "Bob"), "Alien");
    assert_eq!(
        rep_right.effective_role_at(&h_right.roster, "Carol"),
        "Synthetic Alien"
    );
    assert_eq!(rep_right.effective_role_at(&h_right.roster, "Dave"), "Zerb");

    let frank_plain = Text::parse(&rep_right.dms_to("Frank")).plain();
    assert!(
        frank_plain.contains("Players who viewed or moved a card tonight: Alice, Bob, Carol"),
        "got: {frank_plain}"
    );
    assert!(
        !frank_plain.contains("Alice, Bob, Carol, Dave"),
        "got: {frank_plain}"
    );

    // 2. PassLeft rotates unshielded waking Aliens one step LEFT (Alice -> Carol -> Bob -> Alice):
    let mut h_left = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &SYNTHETIC_ALIEN),
            ("Carol", &GROOB),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_left.set_key(ForcedAlienEvent(Some(AlienEvent::PassLeft)));
    let n_left = h_left.spawn_night();
    let rep_left = h_left.finish_night(n_left).await;

    assert_eq!(
        rep_left.effective_role_at(&h_left.roster, "Alice"),
        "Synthetic Alien"
    );
    assert_eq!(rep_left.effective_role_at(&h_left.roster, "Bob"), "Groob");
    assert_eq!(rep_left.effective_role_at(&h_left.roster, "Carol"), "Alien");

    // 3. Fewer than 2 unshielded waking Aliens on PassLeft/PassRight AND solo StandardRecon are no-ops
    //    (no center peek prompt and zero Aura Seer views):
    for ev in [
        AlienEvent::StandardRecon,
        AlienEvent::PassLeft,
        AlienEvent::PassRight,
    ] {
        let mut h_solo = OnuwTestHarness::new(
            &[
                ("Alice", &ALIEN),
                ("Bob", &AURA_SEER),
                ("Carol", &VILLAGER),
            ],
            [&SEER, &VILLAGER, &VILLAGER],
        )
        .await;
        h_solo.set_key(ForcedAlienEvent(Some(ev)));
        let ns = h_solo.spawn_night();
        let reps = h_solo.finish_night(ns).await;
        assert!(!reps.dms_to("Alice").contains("Center #"));
        assert!(reps
            .dms_to("Bob")
            .contains("Nobody else viewed or moved any cards"));
    }
}

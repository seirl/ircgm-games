use crate::roles::base::{
    doppelganger::DOPPELGANGER, robber::ROBBER, tanner::TANNER, villager::VILLAGER,
    werewolf::WEREWOLF,
};
use crate::roles::daybreak::paranormal_investigator::PARANORMAL_INVESTIGATOR;
use crate::roles::vampire::copycat::COPYCAT;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn copycat_runs_before_doppelganger_and_handles_doppelganger_and_pi_interactions() {
    // 1. Copycat peeks Werewolf in Center #1; Doppelgänger copies Copycat, sees ONLY "Copycat",
    //    stays asleep, and inherits Werewolf for endgame!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &COPYCAT),
            ("Carol", &VILLAGER),
        ],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Bob", "1").await;
    h.reply_dm("Alice", "Bob").await;
    h.reply_dm("Bob", "pass").await;
    let report = h.finish_night(night).await;
    assert!(!report.channel_lines.join("\n").contains("Dusk falls!"));
    assert!(report.dms_to("Alice").contains("Copycat"));
    assert!(!report.dms_to("Alice").contains("Werewolf"));
    assert_eq!(report.effective_role_at(&h.roster, "Bob"), "Werewolf");
    assert_eq!(report.effective_role_at(&h.roster, "Alice"), "Werewolf");

    // 2. Copycat peeks PI in center, Doppelgänger copies Copycat, then Copycat-PI becomes Tanner -> Doppel stays Village PI!
    let mut h_pi = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &COPYCAT),
            ("Carol", &TANNER),
        ],
        [&PARANORMAL_INVESTIGATOR, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_pi = h_pi.spawn_night();
    h_pi.reply_dm("Bob", "1").await;
    h_pi.reply_dm("Alice", "Bob").await;
    h_pi.reply_dm("Bob", "Carol").await;
    let rep_pi = h_pi.finish_night(n_pi).await;
    assert_eq!(rep_pi.effective_role_at(&h_pi.roster, "Bob"), "Tanner");
    assert_eq!(
        rep_pi.effective_role_at(&h_pi.roster, "Alice"),
        "Paranormal Investigator"
    );

    // 3. Copycat viewing Doppelgänger in center wakes at #1 (respecting #0 Sentinel shields),
    //    copies Robber (DgImmediate), steals Werewolf, keeps printed Copycat face on swapped card,
    //    and wins on the Werewolf team!
    use crate::roles::base::seer::SEER;
    use crate::roles::daybreak::sentinel::SENTINEL;
    let mut h_cc = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &ROBBER),
            ("Carol", &WEREWOLF),
            ("Dave", &SENTINEL),
            ("Eve", &SEER),
        ],
        [&DOPPELGANGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_cc = h_cc.spawn_night();
    // #-8: Alice (Copycat) views Center #1 (Doppelgänger)
    h_cc.reply_dm("Alice", "1").await;
    // #0: Dave (Sentinel) shields Eve
    h_cc.reply_dm("Dave", "Eve").await;
    // #1: Alice wakes as Copycat-Doppelgänger; copying shielded Eve is rejected, so Alice copies Bob (Robber)
    let dg_prompt = loop {
        let m = h_cc.wait_for_new_dm("Alice").await;
        if m.contains("🎭 Doppelgänger:") {
            break m;
        }
    };
    assert!(dg_prompt.contains("Pick ONE other player"));
    h_cc.send_dm("Alice", "Eve").await;
    let shield_reject = h_cc.wait_for_new_dm("Alice").await;
    assert!(shield_reject.contains("shielded by the Sentinel"));
    h_cc.send_dm("Alice", "Bob").await;
    // #1 (DoppelgangerImmediate): Alice immediately acts as Robber and steals Carol's Werewolf card!
    h_cc.reply_dm("Alice", "Carol").await;
    // #2: Carol (initial Werewolf) passes Lone Wolf peek; #5: Eve (Seer) peeks Carol's swapped card; #6: Bob passes
    h_cc.reply_dm("Carol", "pass").await;
    h_cc.reply_dm("Eve", "Carol").await;
    h_cc.reply_dm("Bob", "pass").await;
    let rep_cc = h_cc.finish_night(n_cc).await;
    assert!(rep_cc
        .dms_to("Alice")
        .contains("Stand by—your copied role's action will fire on schedule!"));
    assert_eq!(rep_cc.effective_role_at(&h_cc.roster, "Alice"), "Werewolf");
    assert_eq!(rep_cc.effective_role_at(&h_cc.roster, "Carol"), "Robber");
    // Carol now holds Alice's physical Copycat card (whose printed face is still Copycat when peeked by Seer!):
    assert!(rep_cc.dms_to("Eve").contains("Carol's card is: Copycat"));
    assert!(rep_cc.has_chronicle_substr("Copycat-Doppelgänger (Alice) copied Bob and became Robber."));
    assert!(rep_cc.has_chronicle_substr(
        "Copycat-Robber (Alice) stole Werewolf from Carol (giving Carol Copycat)."
    ));
    let dawn_cc = h_cc
        .resolve_voting(
            rep_cc,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
                ("Eve", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn_cc.winner_nicks(&h_cc.roster), vec!["Alice"]);
}

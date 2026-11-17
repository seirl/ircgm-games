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

    // 3. Copycat viewing Doppelgänger in center stays asleep:
    let mut h_cc = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &ROBBER),
            ("Carol", &VILLAGER),
        ],
        [&DOPPELGANGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_cc = h_cc.spawn_night();
    h_cc.reply_dm("Alice", "1").await;
    h_cc.reply_dm("Bob", "pass").await;
    let rep_cc = h_cc.finish_night(n_cc).await;
    assert_eq!(rep_cc.effective_role_at(&h_cc.roster, "Alice"), "Doppelgänger");
    assert!(!rep_cc.dms_to("Alice").contains("🎭 Doppelgänger:"));
}

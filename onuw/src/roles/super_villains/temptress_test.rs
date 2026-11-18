use crate::roles::base::villager::VILLAGER;
use crate::roles::super_villains::{henchman::HENCHMAN, temptress::TEMPTRESS};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn temptress_excludes_waking_villains_plants_extra_villain_and_notifies_syndicate() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &HENCHMAN),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    let report = h.finish_night(night).await;

    assert!(report.dms_to("Alice").contains("Non-Villains: Carol, Dave"));
    assert!(report
        .dms_to("Bob")
        .contains("planted the extra Villain card onto Carol"));
    assert_eq!(report.effective_role_at(&h.roster, "Carol"), "Dr. Peeker");
}

#[tokio::test(start_paused = true)]
async fn doppelganger_temptress_runs_sequentially_and_hunted_oracle_in_extra_slot_wipes_winners() {
    use crate::roles::{
        alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE},
        base::doppelganger::DOPPELGANGER,
    };

    // 1. Temptress (Alice) + Doppelgänger-Temptress (Bob) both target Carol sequentially:
    //    Alice plants Villain onto Carol (Carol's Villager moves to extra slot);
    //    Bob then swaps Carol's old Villager back onto Carol without lying to the Syndicate!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &TEMPTRESS),
            ("Bob", &DOPPELGANGER),
            ("Carol", &VILLAGER),
            ("Dave", &HENCHMAN),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n = h.spawn_night();
    h.reply_dm("Bob", "Alice").await;
    h.reply_dm("Alice", "Carol").await;
    h.reply_dm("Bob", "Carol").await;
    let rep = h.finish_night(n).await;
    assert_eq!(rep.effective_role_at(&h.roster, "Carol"), "Villager");
    assert!(rep
        .dms_to("Dave")
        .contains("swapped the perpendicular card onto Carol"));

    // 2. Temptress swapping onto Oracle [Hunted] puts Oracle [Hunted] into the Extra Villain slot -> nobody wins!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &TEMPTRESS),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 9,
    })));
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "1").await;
    h2.reply_dm("Bob", "Alice").await;
    let rep2 = h2.finish_night(n2).await;
    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn2.winner_nicks(&h2.roster).is_empty());
}

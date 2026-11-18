use crate::roles::base::villager::VILLAGER;
use crate::roles::super_villains::{
    henchman::HENCHMAN,
    mad_scientist_intern::{INTERN, MAD_SCIENTIST},
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn mad_scientist_and_intern_co_win_blocking_all_teams_even_when_villain_also_dies() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &HENCHMAN),
            ("Bob", &MAD_SCIENTIST),
            ("Carol", &INTERN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Carol").contains("The Mad Scientist is Bob"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Carol"]);
}

#[tokio::test(start_paused = true)]
async fn vampire_bitten_mad_scientist_leaves_intern_responsible_for_the_lab() {
    use crate::roles::vampire::vampire::VAMPIRE;

    // 1. Bob (Mad Scientist) receives Mark::Vampire in Dusk, becoming a Vampire threat (no active
    //    Mad Scientist remains). Executing Bob kills a Vampire: Village (Dave) wins, while surviving
    //    Intern (Carol) neither wins nor blocks Village.
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &MAD_SCIENTIST),
            ("Carol", &INTERN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Carol").contains("The Mad Scientist is Bob"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Dave"]);

    // 2. Same setup, but Carol (Intern) gets executed instead: because no player is an active
    //    Mad Scientist, Intern wins solo just like Mad Scientist and blocks all teams.
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &MAD_SCIENTIST),
            ("Carol", &INTERN),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    let report2 = h2.finish_night(night2).await;

    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol"]);
}

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
async fn intern_wins_when_vampire_bitten_mad_scientist_is_killed() {
    use crate::roles::vampire::vampire::VAMPIRE;

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
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Carol"]);
}

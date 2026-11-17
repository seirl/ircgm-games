use crate::roles::base::{tanner::TANNER, villager::VILLAGER};
use crate::roles::daybreak::witch::WITCH;
use crate::roles::vampire::{
    assassin::{APPRENTICE_ASSASSIN, ASSASSIN},
    instigator::INSTIGATOR,
    vampire::VAMPIRE,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn assassin_and_apprentice_assassin_mutual_dusk_sight_bitten_mentor_and_center_swap_rules() {
    // 1. Mutual Dusk sight at #-1B:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ASSASSIN),
            ("Bob", &APPRENTICE_ASSASSIN),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n = h.spawn_night();
    h.reply_dm("Alice", "Carol").await;
    let rep = h.finish_night(n).await;
    assert!(rep.dms_to("Bob").contains("Your mentor Assassin is Alice"));
    assert!(rep
        .dms_to("Alice")
        .contains("Your Apprentice Assassin is Bob"));

    // 2. Apprentice Assassin wins when killing a Mark::Vampire-bitten Assassin:
    let mut h_aa = OnuwTestHarness::new(
        &[
            ("Alice", &VAMPIRE),
            ("Bob", &ASSASSIN),
            ("Carol", &APPRENTICE_ASSASSIN),
            ("Dave", &INSTIGATOR),
            ("Eve", &TANNER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_aa = h_aa.spawn_night();
    h_aa.reply_dm("Alice", "Bob").await;
    h_aa.reply_dm("Dave", "Alice").await;
    h_aa.reply_dm("Bob", "Dave").await;
    let rep_aa = h_aa.finish_night(n_aa).await;
    let dawn_aa = h_aa
        .resolve_voting(
            rep_aa,
            &[
                ("Alice", Some("Bob")),
                ("Carol", Some("Bob")),
                ("Bob", Some("Eve")),
                ("Dave", Some("Eve")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_aa.winner_nicks(&h_aa.roster),
        vec!["Alice", "Carol", "Dave", "Eve"]
    );

    // 3. Witch swapping Assassin into center prevents Apprentice Assassin from winning off Assassin's mark:
    let mut h_sw = OnuwTestHarness::new(
        &[
            ("Alice", &ASSASSIN),
            ("Bob", &APPRENTICE_ASSASSIN),
            ("Carol", &WITCH),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_sw = h_sw.spawn_night();
    h_sw.reply_dm("Alice", "Carol").await;
    h_sw.reply_dm("Carol", "1").await;
    h_sw.reply_dm("Carol", "Alice").await;
    let rep_sw = h_sw.finish_night(n_sw).await;
    let dawn_sw = h_sw
        .resolve_voting(
            rep_sw,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert!(dawn_sw.winner_nicks(&h_sw.roster).is_empty());
}

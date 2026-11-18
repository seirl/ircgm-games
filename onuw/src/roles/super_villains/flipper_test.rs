use crate::roles::super_villains::{
    flipper::FLIPPER, henchman::HENCHMAN, innocent_bystander::INNOCENT_BYSTANDER,
    the_sponge::THE_SPONGE,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn flipper_leaves_hero_face_up_and_flips_innocent_bystander_and_villain_back_face_down() {
    // 1. Hero (The Sponge) stays face up:
    let mut h1 = OnuwTestHarness::new(
        &[
            ("Alice", &FLIPPER),
            ("Bob", &THE_SPONGE),
            ("Carol", &HENCHMAN),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let n1 = h1.spawn_night();
    h1.reply_dm("Alice", "Bob").await;
    let rep1 = h1.finish_night(n1).await;
    assert!(rep1.board.is_player_face_up(1));

    // 2. Innocent Bystander flips BACK face down:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &FLIPPER),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &HENCHMAN),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    let rep2 = h2.finish_night(n2).await;
    assert!(rep2.dms_to("Alice").contains("flip it BACK face down"));
    assert!(!rep2.board.is_player_face_up(1));

    // 3. Evil Minion ALSO flips BACK face down for Flipper (unlike Daybreak Revealer):
    use crate::roles::base::minion::MINION;
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &FLIPPER),
            ("Bob", &MINION),
            ("Carol", &HENCHMAN),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let n3 = h3.spawn_night();
    h3.reply_dm("Alice", "Bob").await;
    let rep3 = h3.finish_night(n3).await;
    assert!(rep3.dms_to("Alice").contains("flip it BACK face down"));
    assert!(!rep3.board.is_player_face_up(1));
}

#[tokio::test(start_paused = true)]
async fn innocent_bystander_is_independent_for_traitor_nostradamus_and_revealer_while_winning_with_heroes()
{
    use crate::role::Role;
    use crate::roles::{
        alien::nostradamus::NOSTRADAMUS, daybreak::revealer::REVEALER,
        vampire::instigator::INSTIGATOR,
    };

    // 1. Hero (Instigator Alice) marks Hero (The Sponge Bob) with Mark::Traitor:
    //    killing Innocent Bystander (Carol) + Henchman (Dave) wins for Heroes & Innocent Bystander,
    //    while Traitor Bob does NOT win because Innocent Bystander is not on the Hero team!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &INSTIGATOR),
            ("Bob", &THE_SPONGE),
            ("Carol", &INNOCENT_BYSTANDER),
            ("Dave", &HENCHMAN),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    assert!(REVEALER
        .summary_for_deck(&h.roster)
        .contains("Innocent Bystander"));

    let night = h.spawn_night();
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Bob", Some("Carol")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    // Dave (Henchman) & Carol (Innocent Bystander) both die: Hero Alice + Innocent Bystander Carol win,
    // while Traitor Hero Bob loses because no Hero teammate died!
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Carol"]);

    // 2. Nostradamus stops immediately on Innocent Bystander and co-wins when Heroes win!
    let mut h_n = OnuwTestHarness::new(
        &[
            ("Alice", &NOSTRADAMUS),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &HENCHMAN),
            ("Dave", &THE_SPONGE),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    let nn = h_n.spawn_night();
    h_n.reply_dm("Alice", "Bob").await; // Stops immediately on Innocent Bystander (no 2nd peek prompt!)
    let rep_n = h_n.finish_night(nn).await;
    let dawn_n = h_n
        .resolve_voting(
            rep_n,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn_n.winner_nicks(&h_n.roster), vec!["Alice", "Bob", "Dave"]);
}

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
}

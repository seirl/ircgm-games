use crate::roles::super_villains::{
    family_man::{ActiveFamilyShape, FamilyShape, FAMILY_MAN},
    henchman::HENCHMAN,
    innocent_bystander::INNOCENT_BYSTANDER,
};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn family_man_co_wins_when_self_and_family_survive_and_loses_if_family_member_dies() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &FAMILY_MAN),
            ("Bob", &INNOCENT_BYSTANDER),
            ("Carol", &HENCHMAN),
            ("Dave", &INNOCENT_BYSTANDER),
        ],
        [
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
            &INNOCENT_BYSTANDER,
        ],
    )
    .await;
    h.set_key(ActiveFamilyShape(Some(FamilyShape::Right1))); // Alice (#0)'s family = {Alice, Bob (#1)}

    let night = h.spawn_night();
    let report = h.finish_night(night).await;

    // Carol (Henchman #7) dies while Alice & Bob survive -> Heroes + Family Man Alice win!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Dave"]);
}

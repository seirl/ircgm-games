use crate::roles::base::{mason::MASON, villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn two_masons_recognize_each_other_while_solo_mason_learns_partner_is_in_center() {
    let mut h = OnuwTestHarness::new(
        &[("Alice", &MASON), ("Bob", &MASON), ("Carol", &VILLAGER)],
        [&WEREWOLF, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    let report = h.finish_night(night).await;
    assert!(report.dms_to("Alice").contains("Fellow Mason(s): Bob"));
    assert!(report.dms_to("Bob").contains("Fellow Mason(s): Alice"));
    assert!(report.has_chronicle_substr("Masons (Alice, Bob) recognized each other."));

    let mut h2 = OnuwTestHarness::new(
        &[("Alice", &MASON), ("Bob", &VILLAGER), ("Carol", &VILLAGER)],
        [&MASON, &WEREWOLF, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    let report2 = h2.finish_night(night2).await;
    assert!(report2.dms_to("Alice").contains("ONLY Mason"));
    assert!(report2
        .has_chronicle_substr("Mason (Alice) woke alone (the other Mason was in the center)."));
}

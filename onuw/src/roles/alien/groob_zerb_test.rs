use ircgm::prelude::Text;

use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    groob_zerb::{GROOB, ZERB},
    leader::LEADER,
};
use crate::roles::base::{doppelganger::DOPPELGANGER, seer::SEER, villager::VILLAGER};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn groob_and_zerb_rival_win_solo_fallback_and_doppelganger_pointing_rules() {
    // 1. Both Groob & Zerb in play: killing Zerb defeats regular Alien while Groob wins!
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let night = h.spawn_night();
    let report = h.finish_night(night).await;
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
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Bob", "Dave"]);

    // 2. Solo Groob (Zerb in center) gets Lone Alien center peek and plays on Team::Alien:
    let mut h1 = OnuwTestHarness::new(
        &[("Alice", &GROOB), ("Bob", &VILLAGER), ("Carol", &VILLAGER)],
        [&ZERB, &SEER, &VILLAGER],
    )
    .await;
    h1.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let night1 = h1.spawn_night();
    h1.reply_dm("Alice", "2").await;
    let report1 = h1.finish_night(night1).await;
    assert!(report1.dms_to("Alice").contains("Center #2: Seer"));
    let dawn1 = h1
        .resolve_voting(
            report1,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Bob")),
                ("Carol", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(dawn1.winner_nicks(&h1.roster), vec!["Alice"]);

    // 3. Doppelgänger-Groob `1.b.i` & `1.b.ii` pointing vectors:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &LEADER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedAlienEvent(Some(AlienEvent::BioScan)));
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await;
    h2.reply_dm("Alice", "!pass").await;
    let rep2 = h2.finish_night(n2).await;
    let dave2 = Text::parse(&rep2.dms_to("Dave")).plain();
    assert!(
        dave2.contains("Alice → Carol")
            && dave2.contains("Bob → Alice, Carol (both hands)")
            && dave2.contains("Carol → Alice, Bob (both hands)"),
        "Dave DMs: {dave2}"
    );

    // 4. Copycat + Mirror Man both copying center Groob with no Zerb in deck learn they are Alien allies:
    use crate::roles::{super_villains::mirror_man::MIRROR_MAN, vampire::copycat::COPYCAT};
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &COPYCAT),
            ("Bob", &MIRROR_MAN),
            ("Carol", &VILLAGER),
        ],
        [&GROOB, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n3 = h3.spawn_night();
    h3.reply_dm("Alice", "1").await;
    h3.reply_dm("Bob", "1").await;
    let rep3 = h3.finish_night(n3).await;
    assert!(rep3
        .dms_to("Alice")
        .contains("you are all on the regular Alien team"));
    assert!(!rep3.dms_to("Alice").contains("mortal nemesis"));
}

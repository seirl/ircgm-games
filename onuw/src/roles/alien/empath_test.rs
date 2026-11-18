use crate::roles::alien::empath::{EmpathQuestion, ForcedEmpathQuestion, EMPATH};
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn empath_observes_closed_eye_table_poll_across_all_other_players() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER),
            ("Bob", &EMPATH),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedEmpathQuestion(Some(EmpathQuestion::MostSuspicious)));

    let night = h.spawn_night();
    h.reply_dm("Carol", "1").await;
    h.wait_for_prompt_dm("Alice").await;
    h.wait_for_prompt_dm("Carol").await;
    h.wait_for_prompt_dm("Dave").await;
    h.send_dm("Alice", "Carol").await;
    h.send_dm("Carol", "Alice").await;
    h.send_dm("Dave", "Carol").await;

    let report = h.finish_night(night).await;
    let bob_dms = report.dms_to("Bob");
    assert!(
        bob_dms.contains("Alice → Carol, Carol → Alice, Dave → Carol"),
        "got:\n{bob_dms}"
    );

    // Empath + Doppelgänger-Empath: all N players participate in the eyes-closed poll so neither
    // Empath nor Doppelgänger-Empath leaks via a missing arrow, and Oracle logs their wakes separately:
    use crate::roles::alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE};
    use crate::roles::base::{doppelganger::DOPPELGANGER, mason::MASON};
    use ircgm::prelude::Text;

    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &MASON),
            ("Bob", &EMPATH),
            ("Carol", &DOPPELGANGER),
            ("Dave", &ORACLE),
            ("Eve", &DOPPELGANGER),
        ],
        [&MASON, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 2,
    })));
    h2.set_key(ForcedEmpathQuestion(Some(EmpathQuestion::MostTrustworthy)));

    let n2 = h2.spawn_night();
    h2.reply_dm("Dave", "2").await;
    h2.reply_dm("Carol", "Alice").await; // Carol -> Doppelgänger-Mason (joins Mason group wake!)
    h2.reply_dm("Eve", "Bob").await; // Eve -> Doppelgänger-Empath (wakes separately at #10-E-A!)

    for (nick, target) in [
        ("Alice", "Carol"),
        ("Bob", "Alice"),
        ("Carol", "Alice"),
        ("Dave", "Bob"),
        ("Eve", "Dave"),
    ] {
        h2.wait_for_prompt_dm(nick).await;
        h2.send_dm(nick, target).await;
    }

    let rep2 = h2.finish_night(n2).await;
    let bob_plain = Text::parse(&rep2.dms_to("Bob")).plain();
    let eve_plain = Text::parse(&rep2.dms_to("Eve")).plain();
    let dave_watch = Text::parse(&rep2.dms_to("Dave")).plain();
    assert!(
        bob_plain.contains("Alice → Carol, Carol → Alice, Dave → Bob, Eve → Dave"),
        "got:\n{bob_plain}"
    );
    assert!(
        eve_plain.contains("Alice → Carol, Bob → Alice, Carol → Alice, Dave → Bob"),
        "got:\n{eve_plain}"
    );
    assert!(
        dave_watch.contains("Mason: Alice, Carol")
            && dave_watch.contains("Empath: Bob · Doppelgänger-Empath: Eve"),
        "got:\n{dave_watch}"
    );
}

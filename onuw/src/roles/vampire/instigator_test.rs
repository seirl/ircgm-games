use crate::marks::Mark;
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::vampire::instigator::INSTIGATOR;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn instigator_mark_of_traitor_wins_only_when_teammate_dies() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &INSTIGATOR),
            ("Bob", &WEREWOLF),
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    // Alice places Mark::Traitor on Werewolf Bob:
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;
    assert_eq!(report.mark_at(&h.roster, "Bob"), Mark::Traitor);

    // Carol (fellow Werewolf) dies -> Village wins AND Traitor Bob wins!
    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob", "Dave"]);

    // 2. Tanner + Apprentice Tanner share a 2-player Tanner team under Mark::Traitor,
    //    and Mark::Vampire overrides Oracle [Hunted]!
    use crate::roles::{
        alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE},
        base::tanner::TANNER,
        vampire::{apprentice_tanner::APPRENTICE_TANNER, vampire::VAMPIRE},
    };
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &INSTIGATOR),
            ("Bob", &TANNER),
            ("Carol", &APPRENTICE_TANNER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    h2.reply_dm("Alice", "Bob").await; // Tanner Bob gets Mark::Traitor
    let rep2 = h2.finish_night(n2).await;
    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Carol")),
            ],
        )
        .await;
    // Carol (Apprentice Tanner) died -> Carol wins (sole AT? No, Tanner Bob was in play so Carol needed Bob to die!),
    // AND Bob (Tanner + Mark::Traitor) wins because his Tanner-team partner Carol died!
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Bob"]);

    // 3. Oracle [Hunted] bitten by Mark::Vampire converts to Vampire (cancelling Hunted global override):
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedOracleQuestion(Some(OracleQuestion::NumberGuess {
        secret: 9,
    })));
    let n3 = h3.spawn_night();
    h3.reply_dm("Alice", "1").await; // Alice becomes Oracle [Hunted] at #-9
    h3.reply_dm("Bob", "Alice").await; // Vampire Bob marks Alice with Mark::Vampire at #-6!
    let rep3 = h3.finish_night(n3).await;
    let dawn3 = h3
        .resolve_voting(
            rep3,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Alice", "Bob"]);
}

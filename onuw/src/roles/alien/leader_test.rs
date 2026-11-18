use crate::roles::alien::{
    alien::{AlienEvent, ForcedAlienEvent, ALIEN},
    groob_zerb::{GROOB, ZERB},
    leader::LEADER,
};
use crate::roles::base::villager::VILLAGER;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn leader_unanimous_alien_point_steals_victory_and_groob_zerb_survival_win_works() {
    // 1. Unanimous Alien vote on Leader steals Alien team victory even when an Alien dies:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &ALIEN),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let night = h.spawn_night();
    let report = h.finish_night(night).await;
    assert!(report
        .dms_to("Carol")
        .contains("The Aliens are Alice, Bob"));

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);

    // 2. Both Groob & Zerb in play -> Leader wins iff neither Groob nor Zerb dies:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &GROOB),
            ("Bob", &ZERB),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h2.set_key(ForcedAlienEvent(Some(AlienEvent::CenterProbe)));
    let night2 = h2.spawn_night();
    h2.reply_dm("Alice", "!pass").await;
    let report2 = h2.finish_night(night2).await;
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Carol"]);

    // 3. Both Groob & Zerb AND Alien in play: unanimous vote (Alien + Groob + Zerb) on Leader
    //    triggers Alien abduction win AND Leader co-wins because Groob & Zerb survived!
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &GROOB),
            ("Carol", &ZERB),
            ("Dave", &LEADER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h3.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n3 = h3.spawn_night();
    let rep3 = h3.finish_night(n3).await;
    let dawn3 = h3
        .resolve_voting(
            rep3,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Alice", "Dave"]);

    // 4. Oracle [Alien Team] is on the Alien team, so her vote is ALSO required for Leader abduction!
    use crate::roles::alien::oracle::{ForcedOracleQuestion, OracleQuestion, ORACLE};
    let mut h4 = OnuwTestHarness::new(
        &[
            ("Alice", &ORACLE),
            ("Bob", &ALIEN),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h4.set_key(ForcedOracleQuestion(Some(OracleQuestion::AlienExchange {
        symp_on_no: true,
    })));
    h4.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n4 = h4.spawn_night();
    h4.reply_dm("Alice", "no").await;
    let rep4 = h4.finish_night(n4).await;
    let dawn4 = h4
        .resolve_voting(
            rep4,
            &[
                ("Alice", Some("Bob")),   // Oracle [Alien Team] did NOT vote for Leader Carol!
                ("Bob", Some("Carol")),   // Alien voted for Leader Carol
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    // Bob (Alien) died and Oracle [Alien Team] didn't point at Leader -> Village (Carol, Dave) wins!
    assert_eq!(dawn4.winner_nicks(&h4.roster), vec!["Carol", "Dave"]);

    // 5. Doppelgänger copying Groob when Zerb is in the center: neither Zerb woke, so both Groob
    //    holders raise thumbs to Leader instead of faking rivalry pointing!
    use crate::roles::base::doppelganger::DOPPELGANGER;
    let mut h5 = OnuwTestHarness::new(
        &[
            ("Alice", &DOPPELGANGER),
            ("Bob", &GROOB),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&ZERB, &VILLAGER, &VILLAGER],
    )
    .await;
    h5.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n5 = h5.spawn_night();
    h5.reply_dm("Alice", "Bob").await;
    let rep5 = h5.finish_night(n5).await;
    assert!(rep5
        .dms_to("Carol")
        .contains("The Aliens are Alice, Bob"));
    assert!(!rep5.dms_to("Carol").contains("pointing"));
}

#[tokio::test(start_paused = true)]
async fn epic_battle_one_vote_circle_tie_leader_monster_cowin_and_groob_zerb_abduction_banner() {
    use crate::marks::Mark;
    use crate::roles::{base::werewolf::WEREWOLF, vampire::vampire::VAMPIRE};

    // (a) 4-player Epic Battle (`Werewolf`, `Vampire`, `2x Villager`) on a 1-1-1-1 circle vote:
    //     all 4 tied 1-vote leaders die -> Village (Carol & Dave) wins!
    let mut h_tie = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF),
            ("Bob", &VAMPIRE),
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_tie = h_tie.spawn_night();
    h_tie.reply_dm("Bob", "Carol").await;
    h_tie.reply_dm("Alice", "pass").await;
    let mut rep_tie = h_tie.finish_night(n_tie).await;
    rep_tie.board.set_mark_for_test(2, Mark::Clarity);
    let dawn_tie = h_tie
        .resolve_voting(
            rep_tie,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn_tie.dead_nicks(&h_tie.roster).len(), 4);
    assert_eq!(
        dawn_tie.winner_nicks(&h_tie.roster),
        vec!["Carol", "Dave"]
    );

    // (b) Epic Battle `[Alien, Leader, Werewolf, Vampire]` where Alien points at Leader (1 vote, tied 2nd),
    //     Vampire points at Alien (1 vote, tied 2nd), and Leader + Werewolf point at Vampire (2 votes, 1st):
    //     Vampire, Alien, and Leader all die — Alien wins via Leader abduction ("even if an alien is killed")
    //     AND surviving Werewolf co-wins because rival monsters died!
    let mut h_eb = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &LEADER),
            ("Carol", &WEREWOLF),
            ("Dave", &VAMPIRE),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_eb.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n_eb = h_eb.spawn_night();
    h_eb.reply_dm("Dave", "Bob").await;
    h_eb.reply_dm("Carol", "pass").await;
    let mut rep_eb = h_eb.finish_night(n_eb).await;
    rep_eb.board.set_mark_for_test(1, Mark::Clarity);
    let dawn_eb = h_eb
        .resolve_voting(
            rep_eb,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Dave")),
                ("Carol", Some("Dave")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_eb.dead_nicks(&h_eb.roster),
        ["Alice".to_owned(), "Bob".to_owned(), "Dave".to_owned()]
            .into_iter()
            .collect()
    );
    assert_eq!(dawn_eb.winner_nicks(&h_eb.roster), vec!["Alice", "Carol"]);

    // (c) `[Groob, Zerb, Leader, 2x Villager]` where both rivals point at Leader while Groob is executed:
    //     Zerb wins alone, Village loses, and the abduction banner does not falsely claim "Aliens ... WIN!"!
    let mut h_gz = OnuwTestHarness::new(
        &[
            ("Alice", &GROOB),
            ("Bob", &ZERB),
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    h_gz.set_key(ForcedAlienEvent(Some(AlienEvent::StandardRecon)));
    let n_gz = h_gz.spawn_night();
    let rep_gz = h_gz.finish_night(n_gz).await;
    let dawn_gz = h_gz
        .resolve_voting(
            rep_gz,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn_gz.winner_nicks(&h_gz.roster), vec!["Bob"]);
    let chan_gz = dawn_gz.channel_text();
    assert!(chan_gz.contains("the Leader is abducted and the Village loses!"));
    assert!(!chan_gz.contains("Aliens abduct the Leader and WIN!"));
}

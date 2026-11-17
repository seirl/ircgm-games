use crate::artifacts::Artifact;
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::curator::CURATOR;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn token_of_mute_allows_gestures_and_punctuation_while_words_forfeit_personal_win_only() {
    let mute_dm = Artifact::TokenOfMute.recipient_dm().plain();
    assert!(mute_dm.contains(
        "You have the Mask of Muting. If you send any public channel message containing letters/numbers before voting ends, you personally lose (even if your team wins)."
    ));

    // 1. Compliant Mute holder + bluffing non-holder both win:
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "pass").await;
    let mut report = h.finish_night(night).await;
    report
        .board
        .insert_artifact_for_test(h.seat_of("Bob"), Artifact::TokenOfMute);

    h.day_chat("Bob", "...").await;
    h.day_chat("Bob", "?!").await;
    h.day_action("Bob", "nods").await;
    h.day_action("Bob", "shrugs.").await;
    h.day_action("Bob", "points at Carol!").await;
    h.day_chat("Bob", "!order").await;
    h.day_chat("Bob", "!ready").await;
    h.day_chat("Bob", "!vote Carol").await;
    h.day_action("Alice", "nods").await;
    h.day_chat("Alice", "Let's vote Carol!").await;

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);
    assert!(!dawn.channel_text().contains("broke the Mask of Muting"));

    // 2. Violating Mute holder (e.g. smuggling extra words in `/me points at ...` or typing letters/numbers)
    //    personally loses at Dawn even though Village team wins!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Carol", "pass").await;
    h2.reply_dm("Alice", "pass").await;
    let mut report2 = h2.finish_night(night2).await;
    report2
        .board
        .insert_artifact_for_test(h2.seat_of("Bob"), Artifact::TokenOfMute);

    h2.day_action("Bob", "points at Carol because wolf").await;
    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Alice"]);
    assert!(dawn2
        .channel_text()
        .contains("Bob broke the Mask of Muting"));
}

#[tokio::test(start_paused = true)]
async fn shroud_of_shame_enforces_turning_around_and_no_player_nicks_with_word_boundaries() {
    let shame_dm = Artifact::ShroudOfShame.recipient_dm().plain();
    assert!(shame_dm.contains("Shroud of Shame"));
    assert!(shame_dm.contains("/me turns around"));

    // 1. Compliant Shroud holder turns around first and avoids player nicks (note nicks `Eve` & `Rob`
    //    do NOT false-positive on English words `"never"` and `"robbed"`!):
    let mut h = OnuwTestHarness::new(
        &[
            ("Eve", &CURATOR),
            ("Rob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Eve", "pass").await;
    let mut report = h.finish_night(night).await;
    report
        .board
        .insert_artifact_for_test(h.seat_of("Rob"), Artifact::ShroudOfShame);

    // `!order` before `/me turns around` is an exempt public game command (not speech):
    h.day_chat("Rob", "!order").await;
    h.day_action("Rob", "turns around").await;
    h.day_chat(
        "Rob",
        "I never robbed anyone — vote my right neighbor who claimed Seer!",
    )
    .await;
    h.day_chat("Rob", "!ready").await;
    h.day_chat("Rob", "!vote Carol").await;

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Eve", Some("Carol")),
                ("Rob", Some("Carol")),
                ("Carol", Some("Eve")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Eve", "Rob"]);

    // 2. Naming a player (`Carol:`) after turning around forfeits personal win:
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Eve", &CURATOR),
            ("Rob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night2 = h2.spawn_night();
    h2.reply_dm("Carol", "pass").await;
    h2.reply_dm("Eve", "pass").await;
    let mut report2 = h2.finish_night(night2).await;
    report2
        .board
        .insert_artifact_for_test(h2.seat_of("Rob"), Artifact::ShroudOfShame);

    h2.day_action("Rob", "turns around").await;
    h2.day_chat("Rob", "Carol: what role were you?").await;

    let dawn2 = h2
        .resolve_voting(
            report2,
            &[
                ("Eve", Some("Carol")),
                ("Rob", Some("Carol")),
                ("Carol", Some("Eve")),
            ],
        )
        .await;
    assert_eq!(dawn2.winner_nicks(&h2.roster), vec!["Eve"]);
    assert!(dawn2
        .channel_text()
        .contains("Rob broke the Shroud of Shame (mentioned player Carol)"));

    // 3. Never turning around at all also forfeits personal win:
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Eve", &CURATOR),
            ("Rob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night3 = h3.spawn_night();
    h3.reply_dm("Carol", "pass").await;
    h3.reply_dm("Eve", "pass").await;
    let mut report3 = h3.finish_night(night3).await;
    report3
        .board
        .insert_artifact_for_test(h3.seat_of("Rob"), Artifact::ShroudOfShame);

    let dawn3 = h3
        .resolve_voting(
            report3,
            &[
                ("Eve", Some("Carol")),
                ("Rob", Some("Carol")),
                ("Carol", Some("Eve")),
            ],
        )
        .await;
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Eve"]);
    assert!(dawn3
        .channel_text()
        .contains("never faced the wall with /me turns around"));
}

#[tokio::test(start_paused = true)]
async fn day_phase_publicly_announces_which_player_has_an_artifact_token() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "Bob").await;
    let report = h.finish_night(night).await;

    let day_handle = tokio::spawn({
        let room = h.room.clone();
        let roster_seats = h.roster.seats.clone();
        let roster_deck = h.roster.deck.clone();
        let board = report.board.clone();
        async move {
            let r = crate::board::Roster {
                seats: roster_seats,
                deck: roster_deck,
            };
            crate::day::run_day_phase(&room, &r, &board, std::time::Duration::from_secs(30)).await
        }
    });
    h.srv.settle().await;

    let joined = h.srv.take_messages_to("#onuw").join("\n");
    assert!(joined.contains("Artifact Token rests on: Bob"));

    // End day early via channel `!ready` (exempt under TokenOfMute & ShroudOfShame):
    h.srv.chat("Alice", "#onuw", "!ready");
    h.srv.chat("Bob", "#onuw", "!ready");
    h.srv.chat("Carol", "#onuw", "!ready");
    h.srv.settle().await;
    day_handle.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn public_channel_voting_works_and_exempts_shroud_of_shame_and_token_of_mute() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;

    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "pass").await;
    let mut report = h.finish_night(night).await;
    report
        .board
        .insert_artifact_for_test(h.seat_of("Alice"), Artifact::TokenOfMute);
    report
        .board
        .insert_artifact_for_test(h.seat_of("Bob"), Artifact::ShroudOfShame);

    // Bob faces the wall during Day; Alice stays silent:
    h.day_action("Bob", "turns around").await;

    let vote_handle = tokio::spawn({
        let bot = h.bot.clone();
        let room = h.room.clone();
        let roster_seats = h.roster.seats.clone();
        let roster_deck = h.roster.deck.clone();
        let day_log = h.day_channel_log.clone();
        async move {
            let r = crate::board::Roster {
                seats: roster_seats,
                deck: roster_deck,
            };
            let mut log = day_log;
            let mut ci = room.inbox().lossy();
            let votes = crate::day::run_voting_phase(
                &bot,
                &room,
                &r,
                std::time::Duration::from_secs(30),
                None,
            )
            .await
            .unwrap();
            while let Ok(Some(msg)) = ci.try_next_chat() {
                log.push(msg);
            }
            (votes, log)
        }
    });
    h.srv.settle().await;
    let _ = h.srv.take_messages_to("Bob");

    // Trying to vote in DM reminds Bob that voting is public in `#onuw`:
    h.srv.dm("Bob", "!vote Carol");
    h.srv.settle().await;
    let bob_dms = h.srv.take_messages_to("Bob");
    assert!(bob_dms
        .iter()
        .any(|m| m.contains("Voting is public") && m.contains("#onuw")));

    // All three cast `!vote <nick>` publicly in `#onuw` (even with Mute + Shame!):
    h.srv.chat("Alice", "#onuw", "!vote Carol");
    h.srv.chat("Bob", "#onuw", "!vote Carol");
    h.srv.chat("Carol", "#onuw", "!vote Alice");
    h.srv.settle().await;

    let (votes, full_log) = vote_handle.await.unwrap();
    assert_eq!(votes, vec![Some(2), Some(2), Some(0)]);

    let _ = h.srv.take_messages_to("#onuw");
    let outcome = crate::voting::resolve_and_announce_winners(
        &h.room,
        &h.roster,
        report.board,
        report.chronicle,
        votes,
        &full_log,
    )
    .unwrap();
    h.srv.settle().await;

    let winner_nicks: Vec<String> = outcome
        .winners
        .iter()
        .map(|&(s, _)| h.roster.seats[s].user.nick())
        .collect();
    assert_eq!(winner_nicks, vec!["Alice", "Bob"]);
}

#[test]
fn all_twelve_artifacts_have_unique_ids_names_and_backtick_free_dms() {
    assert_eq!(Artifact::ALL.len(), 12);
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for &art in Artifact::ALL {
        assert!(ids.insert(art.id()), "duplicate artifact id {}", art.id());
        assert!(
            names.insert(art.name()),
            "duplicate artifact name {}",
            art.name()
        );
        let dm = art.recipient_dm().plain();
        assert!(
            dm.contains(art.name()),
            "DM for {:?} missing its name: {dm}",
            art
        );
        assert!(
            !dm.contains('`'),
            "DM for {:?} must not contain backticks: {dm}",
            art
        );
    }

    assert!(Artifact::ClawOfTheWerewolf.replaces_role_card());
    assert!(Artifact::BrandOfTheVillager.replaces_role_card());
    assert!(Artifact::CudgelOfTheTanner.replaces_role_card());
    assert!(Artifact::BowOfTheHunter.replaces_role_card());
    assert!(Artifact::CloakOfThePrince.replaces_role_card());
    assert!(Artifact::SwordOfTheBodyguard.replaces_role_card());
    assert!(Artifact::MistOfTheVampire.replaces_role_card());
    assert!(Artifact::AlienArtifact.replaces_role_card());

    assert!(!Artifact::DaggerOfTheTraitor.replaces_role_card());
    assert!(!Artifact::TokenOfMute.replaces_role_card());
    assert!(!Artifact::ShroudOfShame.replaces_role_card());
    assert!(!Artifact::VoidOfNothingness.replaces_role_card());
}

#[test]
fn pool_for_deck_gates_monster_tokens_by_factions_present_and_keeps_all_non_monster_tokens() {
    use crate::role::Role;
    use crate::roles::{
        alien::{
            alien::{ALIEN, SYNTHETIC_ALIEN},
            body_snatcher::BODY_SNATCHER,
            groob_zerb::GROOB,
            oracle::ORACLE,
        },
        base::{hunter::HUNTER, minion::MINION, seer::SEER},
        daybreak::sentinel::SENTINEL,
        super_villains::{henchman::HENCHMAN, temptress::TEMPTRESS},
        vampire::{
            copycat::COPYCAT, count::COUNT, gremlin::GREMLIN, marksman::MARKSMAN,
            renfield::RENFIELD, vampire::VAMPIRE,
        },
    };

    let expected_base_nine = [
        Artifact::BrandOfTheVillager,
        Artifact::CudgelOfTheTanner,
        Artifact::BowOfTheHunter,
        Artifact::CloakOfThePrince,
        Artifact::SwordOfTheBodyguard,
        Artifact::DaggerOfTheTraitor,
        Artifact::TokenOfMute,
        Artifact::ShroudOfShame,
        Artifact::VoidOfNothingness,
    ];

    // 1. Super Villains + Daybreak (`Syndicate Heist`): 0 Werewolves, 0 Vampires, 0 Aliens -> exact 9 non-monster tokens!
    let sv_deck: &[&'static dyn Role] = &[
        &TEMPTRESS, &HENCHMAN, &SENTINEL, &CURATOR, &VILLAGER, &VILLAGER,
    ];
    let sv_pool = Artifact::pool_for_deck(sv_deck);
    assert_eq!(sv_pool, expected_base_nine);

    // 2. Werewolves + Vampire-box utility roles (`Gremlin`, `Copycat`, `Marksman`) + Alien-box `Oracle`:
    //    Only Werewolf monster faction is in the deck -> Claw enabled, Mist & Alien Artifact gated OUT!
    let ww_crossover: &[&'static dyn Role] = &[
        &WEREWOLF, &GREMLIN, &COPYCAT, &MARKSMAN, &ORACLE, &CURATOR,
    ];
    let ww_pool = Artifact::pool_for_deck(ww_crossover);
    assert_eq!(ww_pool.len(), 10);
    assert!(ww_pool.contains(&Artifact::ClawOfTheWerewolf));
    assert!(!ww_pool.contains(&Artifact::MistOfTheVampire));
    assert!(!ww_pool.contains(&Artifact::AlienArtifact));

    // 3. Vampires vs. Village (`Night Stalkers`: Vampires + Curator, 0 Werewolves):
    let vamp_deck: &[&'static dyn Role] = &[
        &VAMPIRE, &COUNT, &GREMLIN, &COPYCAT, &CURATOR, &SEER,
    ];
    let vamp_pool = Artifact::pool_for_deck(vamp_deck);
    assert_eq!(vamp_pool.len(), 10);
    assert!(!vamp_pool.contains(&Artifact::ClawOfTheWerewolf));
    assert!(vamp_pool.contains(&Artifact::MistOfTheVampire));
    assert!(!vamp_pool.contains(&Artifact::AlienArtifact));

    // 4. Alien variants (`Alien`, `Groob`, `Body Snatcher`, `Synthetic Alien`) each enable `AlienArtifact`,
    //    while `Minion` enables `Claw` and `Renfield` enables `Mist`:
    for alien_role in [
        &ALIEN as &'static dyn Role,
        &GROOB,
        &BODY_SNATCHER,
        &SYNTHETIC_ALIEN,
    ] {
        let alien_pool = Artifact::pool_for_deck(&[alien_role, &CURATOR, &HUNTER]);
        assert_eq!(alien_pool.len(), 10);
        assert!(alien_pool.contains(&Artifact::AlienArtifact));
        assert!(!alien_pool.contains(&Artifact::ClawOfTheWerewolf));
        assert!(!alien_pool.contains(&Artifact::MistOfTheVampire));
    }
    assert!(Artifact::pool_for_deck(&[&MINION, &CURATOR]).contains(&Artifact::ClawOfTheWerewolf));
    assert!(Artifact::pool_for_deck(&[&RENFIELD, &CURATOR]).contains(&Artifact::MistOfTheVampire));

    // 5. Three-way monster clash (`Werewolf` + `Vampire` + `Alien` + `Curator`) enables all 12 tokens
    //    and `random_unused_for_deck` draws all 12 without replacement!
    let full_deck: &[&'static dyn Role] = &[&WEREWOLF, &VAMPIRE, &ALIEN, &CURATOR];
    let full_pool = Artifact::pool_for_deck(full_deck);
    assert_eq!(full_pool.len(), 12);

    let mut drawn: Vec<Option<Artifact>> = Vec::with_capacity(12);
    for _ in 0..12 {
        let next = Artifact::random_unused_for_deck(full_deck, &drawn);
        assert!(!drawn.contains(&Some(next)));
        drawn.push(Some(next));
    }
    assert_eq!(drawn.len(), 12);
}

#[tokio::test(start_paused = true)]
async fn bow_cloak_and_sword_artifacts_override_roles_and_marks_and_interact_cleanly() {
    use crate::marks::Mark;
    use crate::roles::base::hunter::HUNTER;

    // 1. Cloak of the Prince is immune to vote execution (`blocks_hunter_shot == false`),
    //    but STILL dies when shot by a dying Bow of the Hunter holder!
    let mut h1 = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER), // Cloak of the Prince -> immune to vote, mortal to Bow shot!
            ("Bob", &VILLAGER),   // Bow of the Hunter -> killed by vote, points at Alice!
            ("Carol", &WEREWOLF),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n1 = h1.spawn_night();
    h1.reply_dm("Carol", "pass").await;
    let mut rep1 = h1.finish_night(n1).await;
    rep1.board
        .insert_artifact_for_test(h1.seat_of("Alice"), Artifact::CloakOfThePrince);
    rep1.board
        .insert_artifact_for_test(h1.seat_of("Bob"), Artifact::BowOfTheHunter);

    let dawn1 = h1
        .resolve_voting(
            rep1,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert_eq!(
        dawn1.dead_nicks(&h1.roster),
        ["Alice".to_owned(), "Bob".to_owned()].into_iter().collect()
    );
    assert_eq!(dawn1.winner_nicks(&h1.roster), vec!["Carol"]);

    // 2. Cloak of the Prince overrides Werewolf + Mark::Vampire onto Village and deflects execution,
    //    while Bow of the Hunter on printed Hunter fires a SINGLE Hunter shot on real Werewolf Carol!
    let mut h2 = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF), // Cloak of the Prince -> Village, immune to 2 votes!
            ("Bob", &HUNTER),     // Bow of the Hunter -> Village, single shot on Carol!
            ("Carol", &WEREWOLF), // real Werewolf
            ("Dave", &VILLAGER),
            ("Eve", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n2 = h2.spawn_night();
    let mut rep2 = h2.finish_night(n2).await;
    rep2.board
        .set_mark_for_test(h2.seat_of("Alice"), Mark::Vampire);
    rep2.board
        .insert_artifact_for_test(h2.seat_of("Alice"), Artifact::CloakOfThePrince);
    rep2.board
        .insert_artifact_for_test(h2.seat_of("Bob"), Artifact::BowOfTheHunter);

    let dawn2 = h2
        .resolve_voting(
            rep2,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
                ("Eve", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn2.dead_nicks(&h2.roster),
        ["Bob".to_owned(), "Carol".to_owned()].into_iter().collect()
    );
    assert_eq!(
        dawn2.winner_nicks(&h2.roster),
        vec!["Alice", "Bob", "Dave", "Eve"]
    );
    let chan2 = dawn2.channel_text();
    assert!(chan2.contains("Cloak of the Prince"));
    assert_eq!(
        chan2.matches("HUNTER SHOT!").count(),
        1,
        "printed Hunter + Bow of the Hunter must fire once, not twice:\n{chan2}"
    );

    // 3. Sword of the Bodyguard blocks BOTH vote execution AND Bow of the Hunter death shot!
    let mut h3 = OnuwTestHarness::new(
        &[
            ("Alice", &WEREWOLF), // Sword of the Bodyguard -> Village, points at Carol!
            ("Bob", &VILLAGER),   // Bow of the Hunter -> dies and shoots Carol (blocked by Sword!)
            ("Carol", &WEREWOLF), // protected by Alice's Sword!
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n3 = h3.spawn_night();
    let mut rep3 = h3.finish_night(n3).await;
    rep3.board
        .insert_artifact_for_test(h3.seat_of("Alice"), Artifact::SwordOfTheBodyguard);
    rep3.board
        .insert_artifact_for_test(h3.seat_of("Bob"), Artifact::BowOfTheHunter);

    let dawn3 = h3
        .resolve_voting(
            rep3,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    // Carol had 2 votes (protected by Sword) AND was targeted by Bob's Bow shot (also blocked by Sword!) -> only Bob dies, Werewolf Carol wins!
    assert_eq!(
        dawn3.dead_nicks(&h3.roster),
        ["Bob".to_owned()].into_iter().collect()
    );
    assert_eq!(dawn3.winner_nicks(&h3.roster), vec!["Carol"]);
    assert!(dawn3.channel_text().contains("Sword of the Bodyguard"));
}

#[tokio::test(start_paused = true)]
async fn mist_of_the_vampire_alien_artifact_and_dagger_of_the_traitor_work_across_expansions() {
    use crate::marks::Mark;
    use crate::roles::{
        alien::{alien::ALIEN, leader::LEADER},
        daybreak::cursed::CURSED,
        vampire::{renfield::RENFIELD, the_master::THE_MASTER},
    };

    // 1. Mist of the Vampire turns holder into primary Vampire threat:
    //    - Keeps Renfield on Vampire Team even with all Vampire cards in center;
    //    - Saves The Master with a loyalty vote and converts Cursed when voting for Cursed!
    let mut h_vamp = OnuwTestHarness::new(
        &[
            ("Alice", &THE_MASTER),
            ("Bob", &VILLAGER), // receives Mist of the Vampire -> primary Vampire!
            ("Carol", &RENFIELD),
            ("Dave", &CURSED),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_vamp = h_vamp.spawn_night();
    h_vamp.reply_dm("Alice", "Carol").await; // Vampire coven marks Renfield (who overwrites self with Bat)
    let mut rep_vamp = h_vamp.finish_night(n_vamp).await;
    rep_vamp
        .board
        .insert_artifact_for_test(h_vamp.seat_of("Bob"), Artifact::MistOfTheVampire);

    // Bob (Mist of the Vampire) votes for The Master (Alice, who got 2 votes from Carol & Dave -> saved!),
    // while Alice (The Master) votes for Cursed Dave -> Dave converts to Vampire, 0 deaths -> all 4 win on Vampire Team!
    let dawn_vamp = h_vamp
        .resolve_voting(
            rep_vamp,
            &[
                ("Alice", Some("Dave")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Bob")),
            ],
        )
        .await;
    assert!(dawn_vamp.dead_nicks(&h_vamp.roster).is_empty());
    assert_eq!(
        dawn_vamp.winner_nicks(&h_vamp.roster),
        vec!["Alice", "Bob", "Carol", "Dave"]
    );

    // 2. Alien Artifact turns holder into primary Alien threat (participating in Leader abduction):
    let mut h_alien = OnuwTestHarness::new(
        &[
            ("Alice", &ALIEN),
            ("Bob", &VILLAGER), // receives Alien Artifact -> must ALSO point at Leader for abduction!
            ("Carol", &LEADER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_alien = h_alien.spawn_night();
    h_alien.reply_dm("Alice", "pass").await;
    let mut rep_alien = h_alien.finish_night(n_alien).await;
    rep_alien
        .board
        .insert_artifact_for_test(h_alien.seat_of("Bob"), Artifact::AlienArtifact);

    // Both Alice (Alien) and Bob (Alien Artifact) point at Leader Carol, while Carol & Dave kill Alice ->
    // unanimous Alien-team point on Leader abducts Carol and steals the win despite Alice dying!
    let dawn_alien = h_alien
        .resolve_voting(
            rep_alien,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_alien.dead_nicks(&h_alien.roster),
        ["Alice".to_owned(), "Carol".to_owned()]
            .into_iter()
            .collect()
    );
    assert_eq!(dawn_alien.winner_nicks(&h_alien.roster), vec!["Alice", "Bob"]);

    // 3. Dagger of the Traitor preserves underlying team/Mark (`Mark::Vampire`) and wins iff a teammate dies
    //    (even when both the Traitor and a teammate die in a tie!):
    let mut h_dag = OnuwTestHarness::new(
        &[
            ("Alice", &VILLAGER), // Mark::Vampire + Dagger of the Traitor -> Traitor to Vampire Team!
            ("Bob", &VILLAGER),   // Mist of the Vampire -> Vampire teammate!
            ("Carol", &VILLAGER),
            ("Dave", &VILLAGER),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let n_dag = h_dag.spawn_night();
    let mut rep_dag = h_dag.finish_night(n_dag).await;
    rep_dag
        .board
        .set_mark_for_test(h_dag.seat_of("Alice"), Mark::Vampire);
    rep_dag
        .board
        .insert_artifact_for_test(h_dag.seat_of("Alice"), Artifact::DaggerOfTheTraitor);
    rep_dag
        .board
        .insert_artifact_for_test(h_dag.seat_of("Bob"), Artifact::MistOfTheVampire);

    // Alice and Bob tie 2-2 and BOTH die -> Vampire Bob (Alice's teammate!) died, so Village (Carol, Dave) AND Traitor Alice win!
    let dawn_dag = h_dag
        .resolve_voting(
            rep_dag,
            &[
                ("Alice", Some("Bob")),
                ("Bob", Some("Alice")),
                ("Carol", Some("Bob")),
                ("Dave", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(
        dawn_dag.dead_nicks(&h_dag.roster),
        ["Alice".to_owned(), "Bob".to_owned()].into_iter().collect()
    );
    assert_eq!(
        dawn_dag.winner_nicks(&h_dag.roster),
        vec!["Alice", "Carol", "Dave"]
    );
}

#[tokio::test(start_paused = true)]
async fn conduct_exempts_valid_role_queries_rejects_smuggled_chat_and_handles_utf8_casefold() {
    let mut h = OnuwTestHarness::new(
        &[
            ("Alice", &CURATOR),
            ("Bob", &VILLAGER),
            ("Carol", &WEREWOLF),
        ],
        [&VILLAGER, &VILLAGER, &VILLAGER],
    )
    .await;
    let night = h.spawn_night();
    h.reply_dm("Carol", "pass").await;
    h.reply_dm("Alice", "pass").await;
    let mut report = h.finish_night(night).await;
    report
        .board
        .insert_artifact_for_test(h.seat_of("Alice"), Artifact::TokenOfMute);
    report
        .board
        .insert_artifact_for_test(h.seat_of("Bob"), Artifact::ShroudOfShame);

    // Valid multi-word `!role mystic wolf` is exempt; smuggling free text in `!role vote Carol` is caught!
    h.day_chat("Alice", "!role mystic wolf").await;
    h.day_action("Bob", "turns around").await;
    // Prefix with `'İ'` (3-byte UTF-8 lowercase fold vs 2-byte uppercase) before mentioning Carol:
    h.day_chat("Bob", "!role İ vote Carol now").await;

    let dawn = h
        .resolve_voting(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    // Alice complied (`!role mystic wolf` is a valid role lookup); Bob broke Shroud of Shame by smuggling `Carol` inside an invalid `!role` command!
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice"]);
    assert!(dawn
        .channel_text()
        .contains("Bob broke the Shroud of Shame (mentioned player Carol)"));
}

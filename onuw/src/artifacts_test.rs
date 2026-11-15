use crate::artifacts::Artifact;
use crate::roles::base::{villager::VILLAGER, werewolf::WEREWOLF};
use crate::roles::daybreak::curator::CURATOR;
use crate::testing::OnuwTestHarness;

#[tokio::test(start_paused = true)]
async fn token_of_mute_allows_gestures_and_punctuation_while_words_forfeit_personal_win_only() {
    let mute_dm = Artifact::TokenOfMute.recipient_dm().plain();
    assert!(mute_dm.contains(
        "You have the Token of Mute. If you send any public channel message containing letters/numbers before voting ends, you personally lose (even if your team wins)."
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
        .artifacts
        .insert(h.seat_of("Bob"), Artifact::TokenOfMute);

    h.day_chat("Bob", "...").await;
    h.day_chat("Bob", "?!").await;
    h.day_action("Bob", "nods").await;
    h.day_action("Bob", "shrugs.").await;
    h.day_action("Bob", "points at Carol!").await;
    h.day_chat("Bob", "!ready").await;
    h.day_chat("Bob", "!vote Carol").await;
    h.day_action("Alice", "nods").await;
    h.day_chat("Alice", "Let's vote Carol!").await;

    let dawn = h
        .resolve_dawn(
            report,
            &[
                ("Alice", Some("Carol")),
                ("Bob", Some("Carol")),
                ("Carol", Some("Alice")),
            ],
        )
        .await;
    assert_eq!(dawn.winner_nicks(&h.roster), vec!["Alice", "Bob"]);
    assert!(!dawn.channel_text().contains("broke the Token of Mute"));

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
        .artifacts
        .insert(h2.seat_of("Bob"), Artifact::TokenOfMute);

    h2.day_action("Bob", "points at Carol because wolf").await;
    let dawn2 = h2
        .resolve_dawn(
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
        .contains("Bob broke the Token of Mute"));
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
        .artifacts
        .insert(h.seat_of("Rob"), Artifact::ShroudOfShame);

    h.day_action("Rob", "turns around").await;
    h.day_chat(
        "Rob",
        "I never robbed anyone — vote my right neighbor who claimed Seer!",
    )
    .await;
    h.day_chat("Rob", "!ready").await;
    h.day_chat("Rob", "!vote Carol").await;

    let dawn = h
        .resolve_dawn(
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
        .artifacts
        .insert(h2.seat_of("Rob"), Artifact::ShroudOfShame);

    h2.day_action("Rob", "turns around").await;
    h2.day_chat("Rob", "Carol: what role were you?").await;

    let dawn2 = h2
        .resolve_dawn(
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
        .contains("Rob broke the Shroud of Shame (mentioned player `Carol`)"));

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
        .artifacts
        .insert(h3.seat_of("Rob"), Artifact::ShroudOfShame);

    let dawn3 = h3
        .resolve_dawn(
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
        .contains("never faced the wall with `/me turns around`"));
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
        .artifacts
        .insert(h.seat_of("Alice"), Artifact::TokenOfMute);
    report
        .board
        .artifacts
        .insert(h.seat_of("Bob"), Artifact::ShroudOfShame);

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
    let outcome = crate::dawn::resolve_and_announce_winners(
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

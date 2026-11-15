use std::collections::HashSet;

use ircgm::testing::test_harness;

use crate::deck::DeckSpec;
use crate::game::GameConfig;
use crate::lobby::run_lobby_bot;
use crate::roles::{find_role, ALL_ROLES};
use crate::scenarios::{
    all_scenarios, find_scenario, scenario_count, scenarios_for_player_count, scenarios_in_pack,
    ScenarioPack,
};

#[test]
fn all_scenarios_are_ordered_by_pack_have_unique_ids_and_valid_deltas_at_every_player_count() {
    assert_eq!(scenario_count(), 59);
    assert_eq!(scenarios_in_pack(ScenarioPack::Base).len(), 8);
    assert_eq!(scenarios_in_pack(ScenarioPack::Daybreak).len(), 8);
    assert_eq!(scenarios_in_pack(ScenarioPack::Vampire).len(), 8);
    assert_eq!(scenarios_in_pack(ScenarioPack::EpicBattle).len(), 27);
    assert_eq!(scenarios_in_pack(ScenarioPack::Alien).len(), 4);
    assert_eq!(scenarios_in_pack(ScenarioPack::SuperVillains).len(), 4);

    let pack_rank = |p: ScenarioPack| -> usize {
        ScenarioPack::ALL
            .iter()
            .position(|&x| x == p)
            .expect("pack in ALL")
    };
    let mut last_rank = 0usize;
    let mut seen_ids = HashSet::new();

    for scen in all_scenarios() {
        let r = pack_rank(scen.pack);
        assert!(
            r >= last_rank,
            "scenario `{}` out of pack order ({:?} after rank {last_rank})",
            scen.id,
            scen.pack
        );
        last_rank = r;

        assert!(
            seen_ids.insert(scen.id),
            "duplicate scenario id `{}`",
            scen.id
        );
        assert_eq!(
            find_scenario(scen.id).map(|s| s.id),
            Some(scen.id),
            "find_scenario failed on canonical id `{}`",
            scen.id
        );

        let (min_p, max_p) = scen.player_range;
        assert!(
            (3..=10).contains(&min_p) && (min_p..=10).contains(&max_p),
            "invalid player_range {:?} on `{}`",
            scen.player_range,
            scen.id
        );

        // Out-of-range player counts must return `Err`:
        if min_p > 3 {
            assert!(scen.build_for_players(min_p - 1).is_err());
        }
        if max_p < 10 {
            assert!(scen.build_for_players(max_p + 1).is_err());
        }

        // Every supported player count `n` in `min_p..=max_p` must build `n + 3` valid cards:
        for n in min_p..=max_p {
            let deck = scen
                .build_for_players(n)
                .unwrap_or_else(|e| panic!("`{}` failed at {n}p: {e}", scen.id));
            assert_eq!(
                deck.len(),
                n + 3,
                "scenario `{}` at {n}p produced {} cards",
                scen.id,
                deck.len()
            );

            // Check `requires_pair_in_deck()` (`Mason`) is always even!
            for &role in ALL_ROLES {
                if role.requires_pair_in_deck() {
                    let count = deck.iter().filter(|&&c| c.id() == role.id()).count();
                    assert_eq!(
                        count % 2,
                        0,
                        "scenario `{}` at {n}p has odd count {count} of paired role `{}`",
                        scen.id,
                        role.name()
                    );
                }
            }

            // Also verify `DeckSpec::from_scenario` builds identically:
            let spec = DeckSpec::from_scenario(scen);
            assert_eq!(spec.build_for_players(n).unwrap().len(), n + 3);
        }
    }
}

#[test]
fn anarchy_scenarios_enforce_max_three_threats_and_never_split_mason_pairs() {
    for anarchy_id in [
        "mg_anarchy",
        "vd_anarchy",
        "eb_anarchy",
        "alien_anarchy",
        "sv_anarchy",
    ] {
        let scen = find_scenario(anarchy_id).expect("anarchy scenario exists");
        assert!(scen.is_anarchy());

        for n in 3..=10 {
            for _trial in 0..40 {
                let deck = scen.build_for_players(n).unwrap();
                assert_eq!(deck.len(), n + 3);

                let threats = deck.iter().filter(|&&r| r.is_primary_threat()).count();
                assert!(
                    threats <= 3,
                    "`{anarchy_id}` at {n}p dealt {threats} monster threats (> 3)"
                );
                assert!(
                    threats >= 1,
                    "`{anarchy_id}` at {n}p dealt 0 monster threats"
                );

                let masons = deck.iter().filter(|&&r| r.id() == "mason").count();
                assert_eq!(masons % 2, 0, "`{anarchy_id}` dealt lone Mason");
            }
        }
    }
}

#[test]
fn fuzzy_lookup_resolves_short_slugs_bilingual_names_and_new_roles() {
    assert_eq!(find_role("The Count").unwrap().id(), "count");
    assert_eq!(find_role("Gremlin").unwrap().id(), "gremlin");
    assert_eq!(find_role("Kobold").unwrap().id(), "gremlin");
    assert_eq!(find_role("Thing").unwrap().id(), "thing");
    assert_eq!(find_role("Prince").unwrap().id(), "prince");

    assert_eq!(
        find_scenario("payback").unwrap().id,
        "rb_base_payback"
    );
    assert_eq!(
        find_scenario("twilight alliance").unwrap().id,
        "rb_base_twilight_alliance"
    );
    assert_eq!(
        find_scenario("werwolfsnacht").unwrap().id,
        "mg_werwolfsnacht"
    );
    assert_eq!(
        find_scenario("Für den König!").unwrap().id,
        "vd_mg_for_the_king"
    );
    assert_eq!(
        find_scenario("from dusk till dawn").unwrap().id,
        "eb_from_dusk_till_dawn"
    );
    assert_eq!(
        find_scenario("final showdown").unwrap().id,
        "vd_all3_final_showdown"
    );

    let eight_p = scenarios_for_player_count(8);
    assert!(eight_p.iter().any(|s| s.id == "rb_base_revenant"));
    assert!(!eight_p.iter().any(|s| s.id == "rb_base_lonely_night"));
}

#[tokio::test(start_paused = true)]
async fn lobby_supports_browsing_scenarios_by_pack_and_enforces_scenario_player_range() {
    let (bot, mut srv) = test_harness("GM");
    srv.complete_join("#onuw", &["Alice", "Bob", "Carol", "Dave"]);
    srv.settle().await;

    let lobby = bot.room("#onuw").unwrap();
    let cfg = GameConfig::for_tests();
    let bot_task = tokio::spawn({
        let bot = bot.clone();
        let lobby = lobby.clone();
        async move { run_lobby_bot(&bot, &lobby, &[], &cfg).await }
    });
    srv.settle().await;
    let _ = srv.take_messages_to("#onuw");

    // 1. Browse `!scenarios`, `!scenarios vampire`, `!scenarios epic 8p`, and `!scenario vd_mg_for_the_king`:
    srv.chat("Alice", "#onuw", "!scenarios");
    srv.chat("Alice", "#onuw", "!scenarios vampire");
    srv.chat("Alice", "#onuw", "!scenarios epic 8p");
    srv.chat("Alice", "#onuw", "!scenario vd_mg_for_the_king");
    srv.settle().await;

    let browse_out = srv.take_messages_to("#onuw").join("\n");
    assert!(browse_out.contains("Preset Scenarios"));
    assert!(browse_out.contains("59 total"));
    assert!(browse_out.contains("vd_first_bite"));
    assert!(browse_out.contains("eb_vampire_revenants"));
    assert!(browse_out.contains("For the King!"));
    assert!(browse_out.contains("Prince"));

    // 2. Open lobby directly on `rb_base_payback` (`4–7p`) and verify `!go` rejects 3 players!
    srv.chat("Alice", "#onuw", "!onuw rb_base_payback");
    srv.settle().await;
    srv.chat("Bob", "#onuw", "!join");
    srv.chat("Carol", "#onuw", "!join");
    srv.chat("Alice", "#onuw", "!go");
    srv.settle().await;

    let reject_out = srv.take_messages_to("#onuw").join("\n");
    assert!(
        reject_out.contains("Need at least 4 players"),
        "expected 4p minimum enforcement, got: {reject_out}"
    );

    // 3. 4th player joins -> host switches scenario to `Lonely Night` (`3–4p`) -> `!abort` cleanly:
    srv.chat("Dave", "#onuw", "!join");
    srv.chat("Alice", "#onuw", "!preset rb_base_lonely_night");
    srv.settle().await;
    let switch_out = srv.take_messages_to("#onuw").join("\n");
    assert!(switch_out.contains("Lonely Night"));

    srv.chat("Alice", "#onuw", "!abort");
    srv.settle().await;

    bot.request_shutdown();
    bot_task.await.unwrap().unwrap();
}

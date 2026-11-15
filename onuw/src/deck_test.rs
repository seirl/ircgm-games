use crate::deck::{parse_role_spec_tokens, DeckSpec};
use crate::roles::find_role;
use crate::scenarios::ScenarioPack;

#[test]
fn deck_spec_supports_packs_multipliers_pairs_and_custom_clear() {
    for pack in ScenarioPack::ALL {
        for n in 3..=10 {
            let spec = DeckSpec::from_pack(pack);
            let deck = spec.build_for_players(n).expect("valid pack deck");
            assert_eq!(deck.len(), n + 3);
        }
    }

    assert!(DeckSpec::parse("chaos").is_some());

    let mut spec = DeckSpec::from_pack(ScenarioPack::Base);
    let parsed = parse_role_spec_tokens("2x alpha wolf, rogue, mason").unwrap();
    for (r, c) in parsed {
        spec.add_role(r, c);
    }
    let deck = spec.build_for_players(5).unwrap();
    assert_eq!(deck.len(), 8);
    assert_eq!(deck.iter().filter(|r| r.id() == "alpha_wolf").count(), 2);
    assert_eq!(deck.iter().filter(|r| r.id() == "rogue").count(), 1);
    assert_eq!(deck.iter().filter(|r| r.id() == "mason").count(), 2);

    spec.clear_to_custom();
    spec.add_role(find_role("werewolf").unwrap(), 1);
    spec.add_role(find_role("seer").unwrap(), 1);
    let custom = spec.build_for_players(3).unwrap();
    assert_eq!(custom.len(), 6);
    assert_eq!(custom.iter().filter(|r| r.id() == "werewolf").count(), 1);
    assert_eq!(custom.iter().filter(|r| r.id() == "seer").count(), 1);
    assert_eq!(custom.iter().filter(|r| r.id() == "villager").count(), 4);
}

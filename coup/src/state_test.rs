use crate::actions::{
    parse_action, parse_block_role, parse_examine_decision, parse_exchange_keep,
    parse_influence_choice, Action, ExamineDecision,
};

use super::{copies_per_role, parse_role, Allegiance, BoardState, GameMode, PlayerState, Role};

#[test]
fn mode_parsing_and_deck_scaling_match_official_rules() {
    assert_eq!(GameMode::parse(""), Some(GameMode::classic()));
    assert_eq!(GameMode::parse("classic"), Some(GameMode::classic()));
    assert_eq!(GameMode::parse("inquisition"), Some(GameMode::inquisitor()));
    assert_eq!(GameMode::parse("inquisitor"), Some(GameMode::inquisitor()));
    assert_eq!(
        GameMode::parse("reformation"),
        Some(GameMode::reformation())
    );
    assert_eq!(
        GameMode::parse("reformation inquisition"),
        Some(GameMode::reformation_inquisitor())
    );
    assert_eq!(
        GameMode::parse("ref+inq"),
        Some(GameMode::reformation_inquisitor())
    );
    assert_eq!(
        GameMode::parse("full"),
        Some(GameMode::reformation_inquisitor())
    );
    assert_eq!(GameMode::parse("ambassador inquisition"), None);
    assert_eq!(GameMode::parse("bogus"), None);

    assert_eq!(copies_per_role(1), None);
    assert_eq!(copies_per_role(2), Some(3));
    assert_eq!(copies_per_role(6), Some(3));
    assert_eq!(copies_per_role(7), Some(4));
    assert_eq!(copies_per_role(8), Some(4));
    assert_eq!(copies_per_role(9), Some(5));
    assert_eq!(copies_per_role(10), Some(5));
    assert_eq!(copies_per_role(11), None);

    assert!(GameMode::classic()
        .active_roles()
        .contains(&Role::Ambassador));
    assert!(!GameMode::classic()
        .active_roles()
        .contains(&Role::Inquisitor));
    assert!(GameMode::inquisitor()
        .active_roles()
        .contains(&Role::Inquisitor));
    assert!(!GameMode::inquisitor()
        .active_roles()
        .contains(&Role::Ambassador));
}

#[test]
fn reformation_allegiance_targeting_foreign_aid_and_all_same_exception() {
    let players = vec![
        PlayerState::new([Role::Duke, Role::Captain], 7, Some(Allegiance::Loyalist)),
        PlayerState::new(
            [Role::Assassin, Role::Contessa],
            2,
            Some(Allegiance::Loyalist),
        ),
        PlayerState::new(
            [Role::Ambassador, Role::Duke],
            2,
            Some(Allegiance::Reformist),
        ),
    ];
    let mut board = BoardState::new(
        GameMode::reformation(),
        players,
        vec![Role::Contessa, Role::Captain, Role::Duke],
        0,
        0,
    );

    assert!(!board.all_alive_same_allegiance());
    // Seat 0 (Loyalist) cannot target Seat 1 (Loyalist), CAN target Seat 2 (Reformist):
    assert!(!board.can_target(0, 1));
    assert!(board.can_target(0, 2));
    assert!(Action::Coup(1).validate(&board, 0).is_err());
    assert!(Action::Coup(2).validate(&board, 0).is_ok());

    // Seat 1 (Loyalist) CANNOT block Seat 0's Foreign Aid, Seat 2 (Reformist) CAN:
    assert!(!board.can_block_foreign_aid(0, 1));
    assert!(board.can_block_foreign_aid(0, 2));

    // Official rule: teammates CAN always challenge each other!
    assert!(board.can_challenge(0, 1));
    assert!(board.can_challenge(0, 2));

    // Official rule: you CAN Convert a teammate (cost 2) to split factions!
    assert!(Action::ConvertOther(1).validate(&board, 0).is_ok());

    // Official rule: Embezzle rejected while Treasury is 0, allowed once Treasury > 0:
    assert!(Action::Embezzle.validate(&board, 0).is_err());
    board.treasury = 2;
    assert!(Action::Embezzle.validate(&board, 0).is_ok());

    // Eliminate Seat 2 -> only Loyalists remain -> targeting & Foreign Aid block restrictions lift!
    board.players[2].reveal_slot(0);
    board.players[2].reveal_slot(1);
    assert!(board.all_alive_same_allegiance());
    assert!(board.can_target(0, 1));
    assert!(board.can_block_foreign_aid(0, 1));
    assert!(Action::Coup(1).validate(&board, 0).is_ok());
}

#[test]
fn mandatory_coup_at_ten_coins_and_zero_coin_steal_legality() {
    let players = vec![
        PlayerState::new([Role::Duke, Role::Captain], 10, None),
        PlayerState::new([Role::Contessa, Role::Assassin], 0, None),
    ];
    let mut board = BoardState::new(
        GameMode::classic(),
        players,
        vec![Role::Ambassador, Role::Duke, Role::Captain],
        0,
        0,
    );

    assert!(board.must_coup(0));
    assert!(Action::Income.validate(&board, 0).is_err());
    assert!(Action::Tax.validate(&board, 0).is_err());
    assert!(Action::Steal(1).validate(&board, 0).is_err());
    assert!(Action::Coup(1).validate(&board, 0).is_ok());

    // Below 10 coins, stealing from a 0-coin target is officially legal:
    board.players[0].coins = 2;
    assert!(Action::Steal(1).validate(&board, 0).is_ok());
    assert!(Action::Assassinate(1).validate(&board, 0).is_err());
    assert!(Action::Coup(1).validate(&board, 0).is_err());
}

#[test]
fn forced_examine_swap_draws_before_returning_old_card() {
    let players = vec![
        PlayerState::new([Role::Inquisitor, Role::Duke], 2, None),
        PlayerState::new([Role::Contessa, Role::Captain], 2, None),
    ];
    // Put only ONE card (Assassin) in deck so if we returned before drawing, Contessa could be redrawn.
    let mut board = BoardState::new(GameMode::inquisitor(), players, vec![Role::Assassin], 0, 0);
    let (old, new_role) = board.force_replace_examined_card(1, 0);
    assert_eq!(old, Role::Contessa);
    assert_eq!(new_role, Role::Assassin);
    assert_eq!(board.players[1].cards[0].role, Role::Assassin);
    assert_eq!(board.deck, vec![Role::Contessa]);
}

#[test]
fn action_block_influence_exchange_and_examine_parsers_work() {
    let resolve = |s: &str| match s.to_ascii_lowercase().as_str() {
        "alice" => Some(0),
        "bob" => Some(1),
        _ => None,
    };

    assert_eq!(
        parse_action("hello everyone", GameMode::classic(), resolve).unwrap(),
        None
    );
    assert_eq!(
        parse_action("!income", GameMode::classic(), resolve).unwrap(),
        Some(Action::Income)
    );
    assert_eq!(
        parse_action("!aid", GameMode::classic(), resolve).unwrap(),
        Some(Action::ForeignAid)
    );
    assert_eq!(
        parse_action("!kill Bob", GameMode::classic(), resolve).unwrap(),
        Some(Action::Assassinate(1))
    );
    assert!(parse_action("!examine Bob", GameMode::classic(), resolve).is_err());
    assert_eq!(
        parse_action("!examine Bob", GameMode::inquisitor(), resolve).unwrap(),
        Some(Action::Examine(1))
    );
    assert_eq!(
        parse_action("!convert", GameMode::reformation(), resolve).unwrap(),
        Some(Action::ConvertSelf)
    );
    assert_eq!(
        parse_action("!convert Bob", GameMode::reformation(), resolve).unwrap(),
        Some(Action::ConvertOther(1))
    );
    assert_eq!(
        parse_action("!embezzle", GameMode::reformation(), resolve).unwrap(),
        Some(Action::Embezzle)
    );

    // Block parsing: bare `!block` auto-infers single blocker, requires role on Steal:
    assert_eq!(
        parse_block_role("!block", Action::ForeignAid, GameMode::classic()).unwrap(),
        Some(Role::Duke)
    );
    assert_eq!(
        parse_block_role("!contessa", Action::Assassinate(1), GameMode::classic()).unwrap(),
        Some(Role::Contessa)
    );
    assert!(parse_block_role("!block", Action::Steal(1), GameMode::classic()).is_err());
    assert_eq!(
        parse_block_role("!block amb", Action::Steal(1), GameMode::classic()).unwrap(),
        Some(Role::Ambassador)
    );
    assert!(parse_block_role("!block amb", Action::Steal(1), GameMode::inquisitor()).is_err());
    assert_eq!(
        parse_block_role("!block inq", Action::Steal(1), GameMode::inquisitor()).unwrap(),
        Some(Role::Inquisitor)
    );

    // Influence choice by number or role name:
    let live = [(0, Role::Duke), (1, Role::Contessa)];
    assert_eq!(parse_influence_choice("2", &live).unwrap(), 1);
    assert_eq!(parse_influence_choice("duke", &live).unwrap(), 0);
    assert!(parse_influence_choice("captain", &live).is_err());

    // Exchange keep by index or duplicate role names:
    let pool = [Role::Duke, Role::Duke, Role::Contessa, Role::Captain];
    assert_eq!(parse_exchange_keep("1 3", &pool, 2).unwrap(), vec![0, 2]);
    assert_eq!(
        parse_exchange_keep("duke, duke", &pool, 2).unwrap(),
        vec![0, 1]
    );
    assert!(parse_exchange_keep("1 1", &pool, 2).is_err());
    assert!(parse_exchange_keep("duke", &pool, 2).is_err());

    assert_eq!(parse_examine_decision("!keep"), Some(ExamineDecision::Keep));
    assert_eq!(
        parse_examine_decision("force"),
        Some(ExamineDecision::ForceSwap)
    );
    assert_eq!(parse_role("inq"), Some(Role::Inquisitor));
    assert_eq!(parse_role("c"), None); // ambiguous Captain/Contessa
}

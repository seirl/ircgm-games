use super::{
    exec_power, hitler_knows_fascists, parse_chancellor_choice, parse_president_discard,
    parse_vote, role_distribution, BoardState, ChancellorChoice, ExecPower, Party, Policy,
    PolicyDeck, Role, WinReason,
};

#[test]
fn official_roles_tracks_and_parsers() {
    assert_eq!(role_distribution(4), None);
    assert_eq!(role_distribution(5), Some((3, 1)));
    assert_eq!(role_distribution(6), Some((4, 1)));
    assert_eq!(role_distribution(7), Some((4, 2)));
    assert_eq!(role_distribution(8), Some((5, 2)));
    assert_eq!(role_distribution(9), Some((5, 3)));
    assert_eq!(role_distribution(10), Some((6, 3)));
    assert_eq!(role_distribution(11), None);
    assert!(hitler_knows_fascists(5) && hitler_knows_fascists(6));
    assert!(!hitler_knows_fascists(7) && !hitler_knows_fascists(10));
    assert_eq!(Role::Hitler.party(), Party::Fascist);

    assert_eq!(exec_power(5, 3), Some(ExecPower::Peek));
    assert_eq!(exec_power(6, 4), Some(ExecPower::Execution));
    assert_eq!(exec_power(7, 2), Some(ExecPower::Investigate));
    assert_eq!(exec_power(8, 3), Some(ExecPower::SpecialElection));
    assert_eq!(exec_power(9, 1), Some(ExecPower::Investigate));
    assert_eq!(exec_power(10, 2), Some(ExecPower::Investigate));

    assert_eq!(parse_president_discard("discard lib"), Ok(Policy::Liberal));
    assert!(parse_president_discard("enact liberal").is_err());
    assert_eq!(
        parse_chancellor_choice("enact fas", true),
        Ok(ChancellorChoice::Enact(Policy::Fascist))
    );
    assert_eq!(
        parse_chancellor_choice("!veto", true),
        Ok(ChancellorChoice::ProposeVeto)
    );
    assert!(parse_chancellor_choice("veto", false).is_err());
    assert!(parse_chancellor_choice("discard liberal", true).is_err());
    assert_eq!(parse_vote("!vote ja!"), Some(true));
    assert_eq!(parse_vote("nein!"), Some(false));
    assert_eq!(parse_vote("maybe"), None);
}

#[test]
fn failed_elections_preserve_term_limits_and_chaos_advances_president_and_clears_limits() {
    let mut board = BoardState::new(7, 0, PolicyDeck::new_shuffled());
    assert_eq!(board.advance_president(), 0);
    assert!(board.record_elected_government(1, Role::Liberal).is_none());
    let _ = board.apply_enacted_policy(Policy::Liberal, false);

    assert_eq!(board.advance_president(), 1);
    assert_eq!(board.eligible_chancellors(), vec![2, 3, 4, 5, 6]);
    assert!(!board.record_failed_election());

    // Failed nomination must NOT overwrite last_elected_president (0) or last_elected_chancellor (1):
    assert_eq!(board.advance_president(), 2);
    assert_eq!(board.eligible_chancellors(), vec![3, 4, 5, 6]);
    assert_eq!(board.term_limited_seats(), vec![0, 1]);

    assert!(!board.record_failed_election());
    assert_eq!(board.advance_president(), 3);
    assert!(board.record_failed_election());
    let _ = board.enact_chaos_policy();

    // After Chaos, Presidency advances to Seat 4 and term limits are cleared:
    assert_eq!(board.advance_president(), 4);
    assert_eq!(board.eligible_chancellors(), vec![0, 1, 2, 3, 5, 6]);
}

#[test]
fn special_election_rotation_and_five_living_player_exception() {
    let mut board = BoardState::new(7, 0, PolicyDeck::new_shuffled());
    assert_eq!(board.advance_president(), 0);
    board.special_president = Some(1);

    // Seat 1 runs Special Election and executes Seat 0 (the caller), then STILL gets their regular turn!
    assert_eq!(board.advance_president(), 1);
    assert!(board.is_special_election);
    assert!(board.execute_player(0, Role::Liberal).is_none());
    assert_eq!(board.advance_president(), 1);
    assert!(!board.is_special_election);

    // Next: Seat 2 calls Special Election on Seat 5, who executes Seat 3 -> resumes at Seat 4:
    assert_eq!(board.advance_president(), 2);
    board.special_president = Some(5);
    assert_eq!(board.advance_president(), 5);
    assert!(board.execute_player(3, Role::Fascist).is_none());
    assert_eq!(board.advance_president(), 4);

    // 5 living players left (1, 2, 4, 5, 6): only last elected Chancellor is term-limited, NOT last Pres!
    assert_eq!(board.alive_count(), 5);
    assert!(board.record_elected_government(5, Role::Liberal).is_none());
    assert_eq!(board.advance_president(), 5);
    assert_eq!(board.eligible_chancellors(), vec![1, 2, 4, 6]);
    assert_eq!(board.advance_president(), 6);
    assert_eq!(board.eligible_chancellors(), vec![1, 2, 4]);
    assert_eq!(board.term_limited_seats(), vec![5]);
}

#[test]
fn veto_advances_tracker_to_chaos_hitler_zone_and_deck_reshuffle() {
    let mut deck = PolicyDeck::from_draw_order([
        Policy::Liberal,
        Policy::Fascist,
        Policy::Fascist,
        Policy::Liberal,
        Policy::Fascist,
    ]);
    let hand = deck.draw_three();
    deck.discard([hand[1], hand[2]]);
    assert_eq!(deck.draw_len(), 2);
    assert_eq!(deck.reshuffle_if_needed(), Some(4));
    assert_eq!(deck.reshuffle_if_needed(), None);

    let mut board = BoardState::new(5, 0, PolicyDeck::from_draw_order([Policy::Fascist; 17]));
    board.fas_policies = 2;
    assert_eq!(board.advance_president(), 0);
    assert!(board.record_elected_government(4, Role::Hitler).is_none());

    let (win, power) = board.apply_enacted_policy(Policy::Fascist, false);
    assert!(win.is_none());
    assert_eq!(power, Some(ExecPower::Peek));
    assert_eq!(
        board.record_elected_government(4, Role::Hitler),
        Some(WinReason::HitlerElectedChancellor { chancellor: 4 })
    );

    // Chaos enacting 5th Fascist policy ignores Execution while unlocking Veto Power:
    board.fas_policies = 4;
    let _ = board.enact_chaos_policy();
    assert_eq!(board.fas_policies, 5);
    assert!(!board.record_failed_election());
    assert!(!board.record_failed_election());
    assert!(board.record_elected_government(1, Role::Liberal).is_none());
    assert!(board.record_consented_veto([Policy::Fascist, Policy::Fascist]));
}

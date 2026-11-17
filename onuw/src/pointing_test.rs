use super::{PointBallot, SeatPair};

#[test]
fn seat_pair_matches_both_orders_and_plurality_beats_later_singleton() {
    let p_ab = SeatPair::new(2, 4).unwrap();
    let p_ba = SeatPair::new(4, 2).unwrap();
    assert_eq!(p_ab, p_ba);
    assert_eq!(SeatPair::new(3, 3), None);

    let mut ballot = PointBallot::new();
    // Seat 0 points at (2, 4) ("Carol Dave"), Seat 1 points at (4, 2) ("Dave Carol"),
    // and Seat 2 points later at (2, 3) ("Carol Eve"):
    ballot.record(0, Some(p_ab));
    ballot.record(1, Some(p_ba));
    ballot.record(2, Some(SeatPair::new(2, 3).unwrap()));

    // (2, 4) has 2 votes vs 1 vote for (2, 3), and actor is the latest voter on (2, 4) (seat 1):
    assert_eq!(ballot.tally(), Some((1, SeatPair::new(2, 4).unwrap())));
}

#[test]
fn changing_pointer_replaces_previous_vote_and_ties_break_by_latest() {
    let mut ballot = PointBallot::new();
    ballot.record(0, Some(3usize));
    ballot.record(1, Some(4usize));
    // Seat 0 changes finger from 3 -> 4, then Seat 1 switches to 2:
    ballot.record(0, Some(4usize));
    ballot.record(1, Some(2usize));

    // 1-1 tie between 4 (seq 2, seat 0) and 2 (seq 3, seat 1) -> latest pointer (seat 1 -> 2) wins:
    assert_eq!(ballot.tally(), Some((1, 2)));
}

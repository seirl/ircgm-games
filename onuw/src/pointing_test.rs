use super::PointBallot;

#[test]
fn plurality_beats_later_singleton_and_records_latest_actor_in_plurality() {
    let mut ballot = PointBallot::new();
    // Seat 0 and Seat 1 both point at seat 4, while Seat 2 points later at seat 3:
    ballot.record(0, Some(4usize));
    ballot.record(1, Some(4usize));
    ballot.record(2, Some(3usize));

    // Target 4 has 2 votes vs 1 vote for target 3, and actor is the latest voter on 4 (seat 1):
    assert_eq!(ballot.tally(), Some((1, 4)));
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

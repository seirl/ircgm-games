use super::SeatSet;

#[test]
fn boundary_seats_zero_and_sixty_three_and_full_all() {
    assert_eq!(SeatSet::all(0), SeatSet::EMPTY);
    assert!(SeatSet::all(0).is_empty());
    assert_eq!(SeatSet::all(0).len(), 0);
    assert_eq!(SeatSet::all(0).first(), None);
    assert_eq!(SeatSet::all(0).only(), None);
    assert_eq!(SeatSet::all(0).choose_random(), None);

    let full = SeatSet::all(64);
    assert_eq!(full.len(), 64);
    assert!(full.contains(0));
    assert!(full.contains(63));
    assert!(!full.contains(64));
    assert_eq!(full.without(100), full);
    assert_eq!(full.first(), Some(0));
    assert_eq!(full.only(), None);
    assert_eq!(full.into_iter().collect::<Vec<_>>(), (0..64).collect::<Vec<_>>());

    let hi = SeatSet::single(63);
    assert_eq!(hi.only(), Some(63));
    assert_eq!(hi.first(), Some(63));
    assert_eq!(hi.choose_random(), Some(63));
    assert_eq!(hi.into_iter().collect::<Vec<_>>(), vec![63]);
}

#[test]
fn set_algebra_and_ascending_iteration() {
    let a = SeatSet::from([5, 1, 5, 3]);
    let b = SeatSet::from([3, 4]);

    assert_eq!(a.len(), 3);
    assert_eq!(a.into_iter().collect::<Vec<_>>(), vec![1, 3, 5]);
    assert_eq!((a | b).into_iter().collect::<Vec<_>>(), vec![1, 3, 4, 5]);
    assert_eq!((a & b).only(), Some(3));
    assert_eq!((a - b).into_iter().collect::<Vec<_>>(), vec![1, 5]);
    assert!(a.intersects(b));
    assert!((a - b).is_disjoint(b));

    let mut m = a;
    assert!(!m.insert(3));
    assert!(m.insert(0));
    assert!(m.remove(5));
    assert!(!m.remove(5));
    assert!(!m.remove(64));
    assert_eq!(m.into_iter().collect::<Vec<_>>(), vec![0, 1, 3]);
}

#[test]
#[should_panic(expected = "exceeds 64-player SeatSet limit")]
fn single_out_of_range_panics() {
    let _ = SeatSet::single(64);
}

#[test]
#[should_panic(expected = "exceeds 64-player SeatSet limit")]
fn all_out_of_range_panics() {
    let _ = SeatSet::all(65);
}

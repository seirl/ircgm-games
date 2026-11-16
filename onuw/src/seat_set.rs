//! Compact 64-player [`SeatSet`] bitset (`Copy`, zero-allocation, ascending seat iteration).

use std::fmt;
use std::iter::FusedIterator;
use std::ops::{BitAnd, BitOr, BitOrAssign, Sub};

/// Compact `Copy` bitset of 0-based player seat indices (`0..64`), ordered ascending on iteration.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SeatSet(u64);

impl SeatSet {
    pub const MAX_SEATS: usize = u64::BITS as usize;
    pub const EMPTY: Self = Self(0);

    #[inline]
    pub const fn single(seat: usize) -> Self {
        assert!(
            seat < Self::MAX_SEATS,
            "seat index exceeds 64-player SeatSet limit"
        );
        Self(1u64 << seat)
    }

    #[inline]
    pub const fn all(player_count: usize) -> Self {
        assert!(
            player_count <= Self::MAX_SEATS,
            "player_count exceeds 64-player SeatSet limit"
        );
        if player_count == Self::MAX_SEATS {
            Self(u64::MAX)
        } else {
            Self((1u64 << player_count) - 1)
        }
    }

    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    #[inline]
    pub const fn contains(self, seat: usize) -> bool {
        seat < Self::MAX_SEATS && (self.0 & (1u64 << seat)) != 0
    }

    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub const fn is_disjoint(self, other: Self) -> bool {
        (self.0 & other.0) == 0
    }

    #[inline]
    pub const fn with(self, seat: usize) -> Self {
        Self(self.0 | Self::single(seat).0)
    }

    #[inline]
    pub const fn without(self, seat: usize) -> Self {
        if seat < Self::MAX_SEATS {
            Self(self.0 & !(1u64 << seat))
        } else {
            self
        }
    }

    #[inline]
    pub fn insert(&mut self, seat: usize) -> bool {
        let bit = Self::single(seat).0;
        let newly_inserted = (self.0 & bit) == 0;
        self.0 |= bit;
        newly_inserted
    }

    #[inline]
    pub fn remove(&mut self, seat: usize) -> bool {
        if seat >= Self::MAX_SEATS {
            return false;
        }
        let bit = 1u64 << seat;
        let was_present = (self.0 & bit) != 0;
        self.0 &= !bit;
        was_present
    }

    /// Lowest-numbered seat in the set, if non-empty.
    #[inline]
    pub const fn first(self) -> Option<usize> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0.trailing_zeros() as usize)
        }
    }

    /// Returns the sole seat in the set iff `self.len() == 1`.
    #[inline]
    pub const fn only(self) -> Option<usize> {
        if self.0.is_power_of_two() {
            Some(self.0.trailing_zeros() as usize)
        } else {
            None
        }
    }

    /// Uniformly picks one random seat from the set, or `None` if empty.
    pub fn choose_random(self) -> Option<usize> {
        let count = self.len();
        if count == 0 {
            return None;
        }
        let mut bits = self.0;
        for _ in 0..fastrand::usize(..count) {
            bits &= bits - 1;
        }
        Some(bits.trailing_zeros() as usize)
    }

    #[inline]
    pub const fn iter(self) -> SeatIter {
        SeatIter(self.0)
    }
}

impl fmt::Debug for SeatSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(*self).finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SeatIter(u64);

impl Iterator for SeatIter {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        if self.0 == 0 {
            None
        } else {
            let seat = self.0.trailing_zeros() as usize;
            self.0 &= self.0 - 1;
            Some(seat)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count_ones() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for SeatIter {}
impl FusedIterator for SeatIter {}

impl IntoIterator for SeatSet {
    type Item = usize;
    type IntoIter = SeatIter;

    #[inline]
    fn into_iter(self) -> SeatIter {
        SeatIter(self.0)
    }
}

impl FromIterator<usize> for SeatSet {
    #[inline]
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let mut set = Self::EMPTY;
        set.extend(iter);
        set
    }
}

impl Extend<usize> for SeatSet {
    #[inline]
    fn extend<I: IntoIterator<Item = usize>>(&mut self, iter: I) {
        for seat in iter {
            self.insert(seat);
        }
    }
}

impl<const N: usize> From<[usize; N]> for SeatSet {
    #[inline]
    fn from(seats: [usize; N]) -> Self {
        seats.into_iter().collect()
    }
}

impl BitOr for SeatSet {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SeatSet {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SeatSet {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl Sub for SeatSet {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0)
    }
}

#[cfg(test)]
#[path = "seat_set_test.rs"]
mod test;

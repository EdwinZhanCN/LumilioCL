#![allow(clippy::single_range_in_vec_init)]

use super::pieces::{Pieces, gaps, merge};
use std::ops::Range;

fn merged(ranges: &[Range<u64>]) -> Vec<Range<u64>> {
    let mut all = Vec::new();
    for range in ranges {
        merge(&mut all, range.clone());
    }
    all
}

#[test]
fn merging_joins_ranges_that_touch_or_overlap() {
    assert_eq!(merged(&[1..5, 8..10, 5..8]), [1..10]);
    assert_eq!(merged(&[0..50, 150..200, 100..150, 50..100]), [0..200]);
    assert_eq!(merged(&[10..20, 30..40, 15..35]), [10..40]);
    assert_eq!(merged(&[10..20, 0..5]), [0..5, 10..20]);
    assert_eq!(merged(&[10..20, 12..15, 3..3]), [10..20]);
}

#[test]
fn gaps_are_what_the_ranges_leave_out() {
    assert_eq!(gaps(&[10..20], 30), [0..10, 20..30]);
    assert_eq!(gaps(&[], 30), [0..30]);
    assert_eq!(gaps(&[0..30], 30), []);
    assert_eq!(gaps(&[0..10, 20..30], 30), [10..20]);
}

#[test]
fn a_connection_takes_the_back_half_of_the_largest_share() {
    let mut pieces = Pieces::new(100, Vec::new(), 10);
    assert_eq!(pieces.claim(0, 0), Some(0..100));
    let (at, fits) = pieces.reserve(0, 20);
    assert_eq!((at, fits), (0, 20));
    pieces.commit(0..20);

    assert_eq!(pieces.take(1), Some(60..100));
    // The first connection now stops where the second begins.
    assert_eq!(pieces.left(0), 40);
    assert_eq!(pieces.reserve(0, 50), (20, 40));
    assert_eq!(pieces.reserve(0, 1), (60, 0));
}

#[test]
fn a_share_too_small_to_halve_is_not_split() {
    let mut pieces = Pieces::new(100, vec![0..81], 10);
    assert_eq!(pieces.claim(0, 81), Some(81..100));
    assert!(!pieces.has_work());
    assert_eq!(pieces.take(1), None);
}

#[test]
fn what_a_connection_lets_go_of_waits_for_another() {
    let mut pieces = Pieces::new(100, Vec::new(), 10);
    pieces.claim(0, 0);
    assert_eq!(pieces.take(1), Some(50..100));
    let (at, fits) = pieces.reserve(1, 10);
    pieces.commit(at..at + fits as u64);
    pieces.release(1);

    assert!(pieces.has_waiting());
    assert_eq!(pieces.take(2), Some(60..100));
}

#[test]
fn a_connection_reads_on_into_a_range_let_go_of_right_behind_it() {
    let mut pieces = Pieces::new(100, Vec::new(), 10);
    pieces.claim(0, 0);
    assert_eq!(pieces.take(1), Some(50..100));
    pieces.release(1);

    let (_, fits) = pieces.reserve(0, 50);
    assert_eq!(fits, 50);
    assert_eq!(pieces.left(0), 0);
    assert!(pieces.extend(0));
    assert_eq!(pieces.reserve(0, 100), (50, 50));
}

#[test]
fn the_file_is_complete_once_every_range_is_written() {
    let mut pieces = Pieces::new(100, vec![0..40], 10);
    assert!(!pieces.is_complete());
    assert_eq!(pieces.commit(60..100), 80);
    assert_eq!(pieces.commit(40..60), 100);
    assert!(pieces.is_complete());
    assert_eq!(pieces.written(), [0..100]);
}

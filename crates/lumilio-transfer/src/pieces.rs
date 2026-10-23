//! Which bytes of a file are written and which connection writes the rest,
//! for a download over several connections (plan download-engine T3).
//!
//! Merging written ranges and finding the gaps between them are adapted from
//! fast-pull `crates/fast-pull/src/base/merge.rs` and `base/invert.rs`
//! (<https://github.com/fast-down/core>, MIT, Copyright (c) 2025 fast-down;
//! plan download-engine D1). Differences: gaps are never merged over short
//! written ranges, and work is handed out by splitting the largest range a
//! connection still has to write instead of through a work-stealing queue.

use std::ops::Range;

/// Merges `new` into sorted `ranges` that neither overlap nor touch, joining
/// it with every range it overlaps or touches.
pub(super) fn merge(ranges: &mut Vec<Range<u64>>, new: Range<u64>) {
    if new.start >= new.end {
        return;
    }
    let first = ranges.partition_point(|range| range.end < new.start);
    let mut merged = new;
    let mut last = first;
    while last < ranges.len() && ranges[last].start <= merged.end {
        merged.start = merged.start.min(ranges[last].start);
        merged.end = merged.end.max(ranges[last].end);
        last += 1;
    }
    ranges.splice(first..last, [merged]);
}

/// The parts of `0..length` that sorted, non-overlapping `ranges` leave out.
pub(super) fn gaps(ranges: &[Range<u64>], length: u64) -> Vec<Range<u64>> {
    let mut gaps = Vec::new();
    let mut next = 0;
    for range in ranges {
        if range.start > next {
            gaps.push(next..range.start.min(length));
        }
        next = next.max(range.end);
    }
    if next < length {
        gaps.push(next..length);
    }
    gaps
}

/// One connection's share of the file.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Share {
    worker: usize,
    /// The next byte it writes; bytes before it are written or being written.
    next: u64,
    end: u64,
}

/// The ranges of a file: written, being written by a connection, or waiting
/// for one.
#[derive(Debug)]
pub(super) struct Pieces {
    length: u64,
    written: Vec<Range<u64>>,
    shares: Vec<Share>,
    waiting: Vec<Range<u64>>,
    /// A share smaller than twice this is not split.
    smallest: u64,
}

impl Pieces {
    pub(super) fn new(length: u64, written: Vec<Range<u64>>, smallest: u64) -> Self {
        let waiting = gaps(&written, length);
        Self {
            length,
            written,
            shares: Vec::new(),
            waiting,
            smallest: smallest.max(1),
        }
    }

    /// Gives `worker` the waiting range that begins at `start`, which its
    /// request already asked for.
    pub(super) fn claim(&mut self, worker: usize, start: u64) -> Option<Range<u64>> {
        let index = self.waiting.iter().position(|range| range.start == start)?;
        let range = self.waiting.remove(index);
        self.shares.push(Share {
            worker,
            next: range.start,
            end: range.end,
        });
        Some(range)
    }

    /// Work for `worker`: the first waiting range, else the back half of the
    /// largest share another connection has left, if it is worth splitting.
    pub(super) fn take(&mut self, worker: usize) -> Option<Range<u64>> {
        if !self.waiting.is_empty() {
            let start = self.waiting[0].start;
            return self.claim(worker, start);
        }
        let largest = self.splittable()?;
        let share = &mut self.shares[largest];
        let middle = share.next + (share.end - share.next) / 2;
        let back = middle..share.end;
        share.end = middle;
        self.shares.push(Share {
            worker,
            next: back.start,
            end: back.end,
        });
        Some(back)
    }

    /// Whether [`Self::take`] would find work.
    pub(super) fn has_work(&self) -> bool {
        !self.waiting.is_empty() || self.splittable().is_some()
    }

    /// Whether some range waits for a connection.
    pub(super) fn has_waiting(&self) -> bool {
        !self.waiting.is_empty()
    }

    fn splittable(&self) -> Option<usize> {
        self.shares
            .iter()
            .enumerate()
            .filter(|(_, share)| share.end - share.next >= 2 * self.smallest)
            .max_by_key(|(_, share)| share.end - share.next)
            .map(|(index, _)| index)
    }

    /// Reserves up to `count` bytes of `worker`'s share for it to write,
    /// returning where they start and how many fit before its end. Zero
    /// means the share is done, perhaps because another connection took its
    /// back half.
    pub(super) fn reserve(&mut self, worker: usize, count: usize) -> (u64, usize) {
        let Some(share) = self.shares.iter_mut().find(|share| share.worker == worker) else {
            return (0, 0);
        };
        let left = share.end.saturating_sub(share.next);
        let fits = usize::try_from(left).map_or(count, |left| left.min(count));
        let at = share.next;
        share.next += fits as u64;
        (at, fits)
    }

    /// Bytes of `worker`'s share it has not reserved yet.
    pub(super) fn left(&self, worker: usize) -> u64 {
        self.shares
            .iter()
            .find(|share| share.worker == worker)
            .map_or(0, |share| share.end.saturating_sub(share.next))
    }

    /// Grows `worker`'s share over the waiting range right after it, for a
    /// connection whose answer goes on past its share's end.
    pub(super) fn extend(&mut self, worker: usize) -> bool {
        let Some(share) = self.shares.iter_mut().find(|share| share.worker == worker) else {
            return false;
        };
        let Some(index) = self
            .waiting
            .iter()
            .position(|range| range.start == share.end)
        else {
            return false;
        };
        share.end = self.waiting.remove(index).end;
        true
    }

    /// Records reserved bytes as written; returns how many bytes of the file
    /// are written now.
    pub(super) fn commit(&mut self, range: Range<u64>) -> u64 {
        merge(&mut self.written, range);
        self.written_bytes()
    }

    /// Ends `worker`'s share; what it did not reserve waits for another
    /// connection.
    pub(super) fn release(&mut self, worker: usize) {
        let Some(index) = self.shares.iter().position(|share| share.worker == worker) else {
            return;
        };
        let share = self.shares.remove(index);
        if share.next < share.end {
            merge(&mut self.waiting, share.next..share.end);
        }
    }

    pub(super) fn written_bytes(&self) -> u64 {
        self.written
            .iter()
            .map(|range| range.end - range.start)
            .sum()
    }

    pub(super) fn written(&self) -> &[Range<u64>] {
        &self.written
    }

    pub(super) fn is_complete(&self) -> bool {
        self.written.len() == 1 && self.written[0] == (0..self.length)
    }
}

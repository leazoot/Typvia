// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! When the next scheduled sync round is due.
//!
//! Pure timing arithmetic, kept apart from threads and locks so the three
//! behaviours the schedule promises are unit-testable: a local change runs a
//! round within the debounce window, consecutive failures back off
//! exponentially up to the idle interval and reset on success, and a quiet
//! library keeps the plain idle poll. All times are milliseconds on the
//! caller's monotonic clock.

/// Delay before the first background round, so the first frame paints before
/// the connection attempt takes the database lock.
pub const START_DELAY_MS: u64 = 3_000;

/// Baseline interval between rounds when nothing changed locally. Periodic
/// polling stays the pull schedule in v1 — there is no push channel — but
/// local changes no longer wait for it.
pub const IDLE_INTERVAL_MS: u64 = 300_000;

/// How long after a local change the push round runs. Long enough to batch a
/// burst of edits into one round, short enough that "save here, paste on the
/// other machine" feels immediate.
pub const DEBOUNCE_MS: u64 = 3_000;

/// First retry delay after a failed round; doubles per consecutive failure
/// and is capped at [`IDLE_INTERVAL_MS`], so a dead server degrades to the
/// idle cadence instead of hammering or stalling.
pub const BACKOFF_BASE_MS: u64 = 15_000;

/// The schedule state: consecutive failures, the earliest un-synced local
/// change, and when the next idle poll is due.
#[derive(Debug)]
pub struct Cadence {
    failures: u32,
    dirty_since_ms: Option<u64>,
    next_poll_ms: u64,
}

impl Cadence {
    pub fn new(now_ms: u64) -> Self {
        Self {
            failures: 0,
            dirty_since_ms: None,
            next_poll_ms: now_ms.saturating_add(START_DELAY_MS),
        }
    }

    /// A local write was queued. The first change of a burst starts the
    /// debounce window; later ones ride along (the window never extends, so
    /// a steady stream of edits cannot postpone the round forever).
    pub fn note_local_change(&mut self, now_ms: u64) {
        if self.dirty_since_ms.is_none() {
            self.dirty_since_ms = Some(now_ms);
        }
    }

    /// When the next round is due: the debounced change wins over the idle
    /// poll whenever one is pending.
    pub fn next_due_ms(&self) -> u64 {
        match self.dirty_since_ms {
            Some(since) => self.next_poll_ms.min(since.saturating_add(DEBOUNCE_MS)),
            None => self.next_poll_ms,
        }
    }

    pub fn is_due(&self, now_ms: u64) -> bool {
        now_ms >= self.next_due_ms()
    }

    /// A round that started at `started_ms` finished at `now_ms`. Success
    /// resets the backoff and re-arms the idle poll; failure schedules the
    /// exponential retry (a failed push is retried by the backoff, not the
    /// stale debounce). Only a change announced before the round started is
    /// consumed: one queued while the round was on the wire missed the push
    /// batch, and its debounce must survive to trigger the next round.
    pub fn note_round(&mut self, started_ms: u64, now_ms: u64, ok: bool) {
        if let Some(since) = self.dirty_since_ms
            && since <= started_ms
        {
            self.dirty_since_ms = None;
        }
        if ok {
            self.failures = 0;
            self.next_poll_ms = now_ms.saturating_add(IDLE_INTERVAL_MS);
        } else {
            self.failures = self.failures.saturating_add(1);
            self.next_poll_ms = now_ms.saturating_add(self.backoff_ms());
        }
    }

    /// The current retry delay: BACKOFF_BASE × 2^(failures−1), capped at the
    /// idle interval.
    fn backoff_ms(&self) -> u64 {
        let doublings = self.failures.saturating_sub(1).min(16);
        BACKOFF_BASE_MS
            .saturating_mul(1u64 << doublings)
            .min(IDLE_INTERVAL_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_the_launch_delay() {
        let cadence = Cadence::new(1_000);
        assert_eq!(cadence.next_due_ms(), 1_000 + START_DELAY_MS);
        assert!(!cadence.is_due(1_000));
        assert!(cadence.is_due(1_000 + START_DELAY_MS));
    }

    #[test]
    fn a_local_change_runs_within_the_debounce_window() {
        let mut cadence = Cadence::new(0);
        cadence.note_round(10_000, 10_000, true);
        cadence.note_local_change(12_000);
        assert_eq!(cadence.next_due_ms(), 12_000 + DEBOUNCE_MS);
        // A burst does not extend the window: the first change anchors it.
        cadence.note_local_change(14_000);
        assert_eq!(cadence.next_due_ms(), 12_000 + DEBOUNCE_MS);
    }

    #[test]
    fn a_quiet_library_keeps_the_idle_interval() {
        let mut cadence = Cadence::new(0);
        cadence.note_round(10_000, 10_000, true);
        assert_eq!(cadence.next_due_ms(), 10_000 + IDLE_INTERVAL_MS);
    }

    #[test]
    fn failures_back_off_exponentially_capped_and_reset_on_success() {
        let mut cadence = Cadence::new(0);
        let mut now = 10_000;
        let mut delays = Vec::new();
        for _ in 0..7 {
            cadence.note_round(now, now, false);
            delays.push(cadence.next_due_ms() - now);
            now = cadence.next_due_ms();
        }
        assert_eq!(
            delays,
            vec![15_000, 30_000, 60_000, 120_000, 240_000, 300_000, 300_000]
        );
        cadence.note_round(now, now, true);
        assert_eq!(cadence.next_due_ms() - now, IDLE_INTERVAL_MS);
        // The next failure starts the ladder over.
        cadence.note_round(now, now, false);
        assert_eq!(cadence.next_due_ms() - now, 15_000);
    }

    #[test]
    fn a_round_consumes_the_pending_change() {
        let mut cadence = Cadence::new(0);
        cadence.note_local_change(5_000);
        cadence.note_round(5_500, 6_000, false);
        // The retry comes from the backoff, not the stale debounce.
        assert_eq!(cadence.next_due_ms(), 6_000 + BACKOFF_BASE_MS);
    }

    #[test]
    fn a_change_queued_during_a_round_keeps_its_debounce() {
        let mut cadence = Cadence::new(0);
        // The round starts at 10_000; a write lands while it is on the wire.
        cadence.note_local_change(11_000);
        cadence.note_round(10_000, 12_000, true);
        // The missed change still triggers its own debounced round.
        assert_eq!(cadence.next_due_ms(), 11_000 + DEBOUNCE_MS);
    }

    #[test]
    fn the_extreme_failure_count_does_not_overflow() {
        let mut cadence = Cadence::new(0);
        for _ in 0..100 {
            cadence.note_round(0, 0, false);
        }
        assert_eq!(cadence.next_due_ms(), IDLE_INTERVAL_MS);
    }
}

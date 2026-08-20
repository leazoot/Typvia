//! Unlock-failure throttling: failure counting and cooldown are a
//! UI / use-case layer policy, not a cryptographic control.
//!
//! An online guard against repeated master-password guessing: after a run of
//! failures the next attempts are refused for a cooldown window, without even
//! running the (deliberately expensive) Argon2 derivation. It is intentionally
//! in-memory only — the threat model does not promise defense against an
//! attacker who has the database file and can attack the KDF offline; this
//! throttle exists to blunt casual, interactive guessing.

use crate::model::TimestampMs;

/// Consecutive failures tolerated before a cooldown is imposed.
const MAX_ATTEMPTS_BEFORE_COOLDOWN: u32 = 5;
/// How long the cooldown lasts once tripped.
const COOLDOWN_MS: i64 = 30_000;

/// In-memory failure counter with a cooldown window.
#[derive(Debug, Default)]
pub(crate) struct UnlockThrottle {
    consecutive_failures: u32,
    /// When set and still in the future, attempts are refused until this time.
    locked_until: Option<TimestampMs>,
}

impl UnlockThrottle {
    /// Returns the cooldown deadline if attempts are currently refused, else
    /// `None`. A caller in cooldown must not run the derivation.
    pub(crate) fn cooldown_until(&self, now: TimestampMs) -> Option<TimestampMs> {
        match self.locked_until {
            Some(until) if now < until => Some(until),
            _ => None,
        }
    }

    /// Records a failed attempt. Trips the cooldown once the failure run
    /// reaches the threshold, then resets the run so the next window starts
    /// fresh after the cooldown expires.
    pub(crate) fn record_failure(&mut self, now: TimestampMs) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= MAX_ATTEMPTS_BEFORE_COOLDOWN {
            self.locked_until = Some(now.saturating_add(COOLDOWN_MS));
            self.consecutive_failures = 0;
        }
    }

    /// Clears all failure state after a successful unlock.
    pub(crate) fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.locked_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: TimestampMs = 1_700_000_000_000;

    #[test]
    fn does_not_throttle_before_the_threshold() {
        let mut throttle = UnlockThrottle::default();
        for _ in 0..(MAX_ATTEMPTS_BEFORE_COOLDOWN - 1) {
            throttle.record_failure(T0);
        }
        assert_eq!(throttle.cooldown_until(T0), None);
    }

    #[test]
    fn trips_cooldown_at_the_threshold() {
        let mut throttle = UnlockThrottle::default();
        for _ in 0..MAX_ATTEMPTS_BEFORE_COOLDOWN {
            throttle.record_failure(T0);
        }
        assert_eq!(throttle.cooldown_until(T0), Some(T0 + COOLDOWN_MS));
    }

    #[test]
    fn cooldown_expires_after_the_window() {
        let mut throttle = UnlockThrottle::default();
        for _ in 0..MAX_ATTEMPTS_BEFORE_COOLDOWN {
            throttle.record_failure(T0);
        }
        assert_eq!(throttle.cooldown_until(T0 + COOLDOWN_MS), None);
        assert_eq!(throttle.cooldown_until(T0 + COOLDOWN_MS + 1), None);
    }

    #[test]
    fn success_clears_the_cooldown() {
        let mut throttle = UnlockThrottle::default();
        for _ in 0..MAX_ATTEMPTS_BEFORE_COOLDOWN {
            throttle.record_failure(T0);
        }
        throttle.record_success();
        assert_eq!(throttle.cooldown_until(T0), None);
    }
}

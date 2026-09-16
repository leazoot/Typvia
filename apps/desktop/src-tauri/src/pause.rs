// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Pausing automatic expansion for a while. Only the trigger engine pauses:
//! Quick Bar and the Library insert on an explicit action, which a pause must
//! not second-guess. The pause lives in memory — quitting the app ends it.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// When the current pause ends, if one is running.
#[derive(Default)]
pub struct InsertionPause {
    until: Mutex<Option<Instant>>,
}

impl InsertionPause {
    /// Starts (or restarts) a pause of `length` from `now`.
    pub fn pause(&self, now: Instant, length: Duration) {
        if let Ok(mut guard) = self.until.lock() {
            *guard = Some(now + length);
        }
    }

    /// Ends the pause early.
    pub fn resume(&self) {
        if let Ok(mut guard) = self.until.lock() {
            *guard = None;
        }
    }

    /// Time left in the pause at `now`; a pause that has run out is cleared.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        let mut guard = self.until.lock().ok()?;
        match *guard {
            Some(until) if until > now => Some(until - now),
            Some(_) => {
                *guard = None;
                None
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn counts_down_while_paused() {
        let pause = InsertionPause::default();
        let start = Instant::now();
        pause.pause(start, HOUR);
        assert_eq!(
            pause.remaining(start + Duration::from_secs(600)),
            Some(HOUR - Duration::from_secs(600))
        );
    }

    #[test]
    fn ends_by_itself_when_the_time_is_up() {
        let pause = InsertionPause::default();
        let start = Instant::now();
        pause.pause(start, HOUR);
        assert_eq!(pause.remaining(start + HOUR), None);
        assert_eq!(pause.remaining(start), None);
    }

    #[test]
    fn resume_ends_it_early() {
        let pause = InsertionPause::default();
        let start = Instant::now();
        pause.pause(start, HOUR);
        pause.resume();
        assert_eq!(pause.remaining(start), None);
    }

    #[test]
    fn pausing_again_restarts_the_hour() {
        let pause = InsertionPause::default();
        let start = Instant::now();
        pause.pause(start, HOUR);
        let later = start + Duration::from_secs(1800);
        pause.pause(later, HOUR);
        assert_eq!(
            pause.remaining(start + HOUR),
            Some(Duration::from_secs(1800))
        );
    }
}

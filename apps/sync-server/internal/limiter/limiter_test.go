// Typvia sync server
// Copyright (C) 2026 Typvia contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at
// your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
// General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

package limiter

import (
	"testing"
	"time"
)

// fakeClock is a settable clock for deterministic limiter tests.
type fakeClock struct{ t time.Time }

func (c *fakeClock) now() time.Time          { return c.t }
func (c *fakeClock) advance(d time.Duration) { c.t = c.t.Add(d) }
func newFakeClock() *fakeClock               { return &fakeClock{t: time.Unix(1_700_000_000, 0)} }

func TestBucketAllowsBurstThenRejectsUntilRefill(t *testing.T) {
	t.Parallel()
	clock := newFakeClock()
	b := NewBucket(3, 1, clock.now)

	for i := 0; i < 3; i++ {
		if !b.Allow("k") {
			t.Fatalf("request %d within burst must be allowed", i)
		}
	}
	if b.Allow("k") {
		t.Fatal("request beyond burst must be rejected")
	}
	clock.advance(1 * time.Second)
	if !b.Allow("k") {
		t.Fatal("request after refill must be allowed")
	}
	if b.Allow("k") {
		t.Fatal("only one token refilled after one second")
	}
}

func TestBucketKeysAreIndependent(t *testing.T) {
	t.Parallel()
	clock := newFakeClock()
	b := NewBucket(1, 0.1, clock.now)

	if !b.Allow("a") {
		t.Fatal("first request for key a must be allowed")
	}
	if b.Allow("a") {
		t.Fatal("second request for key a must be rejected")
	}
	if !b.Allow("b") {
		t.Fatal("key b must not be affected by key a")
	}
}

func TestBackoffDoublesDelayPerAttempt(t *testing.T) {
	t.Parallel()
	clock := newFakeClock()
	b := NewBackoff(1*time.Second, 8*time.Second, time.Hour, clock.now)

	if !b.Allow("k") {
		t.Fatal("first attempt must be allowed")
	}
	if b.Allow("k") {
		t.Fatal("immediate retry must be rejected")
	}
	clock.advance(1 * time.Second)
	if !b.Allow("k") {
		t.Fatal("attempt after base delay must be allowed")
	}
	// Delay doubled to 2s: 1s later is still blocked, 2s later allowed.
	clock.advance(1 * time.Second)
	if b.Allow("k") {
		t.Fatal("retry within doubled delay must be rejected")
	}
	clock.advance(1 * time.Second)
	if !b.Allow("k") {
		t.Fatal("attempt after doubled delay must be allowed")
	}
}

func TestBackoffCapsAtMaxAndResetsAfterQuiet(t *testing.T) {
	t.Parallel()
	clock := newFakeClock()
	b := NewBackoff(1*time.Second, 2*time.Second, 10*time.Second, clock.now)

	if !b.Allow("k") {
		t.Fatal("first attempt must be allowed")
	}
	for i := 0; i < 3; i++ {
		clock.advance(2 * time.Second)
		if !b.Allow("k") {
			t.Fatalf("attempt %d after capped delay must be allowed", i)
		}
	}
	// Quiet longer than resetAfter clears the escalation back to base.
	clock.advance(11 * time.Second)
	if !b.Allow("k") {
		t.Fatal("attempt after reset window must be allowed")
	}
	clock.advance(1 * time.Second)
	if !b.Allow("k") {
		t.Fatal("delay must be back at base after reset")
	}
}

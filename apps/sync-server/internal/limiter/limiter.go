// Package limiter provides the server's in-memory rate limits: token buckets
// for request smoothing and per-key escalating backoff for the anonymous
// recovery endpoint. State is per-process — acceptable for the v1
// single-binary deployment.
package limiter

import (
	"sync"
	"time"
)

// pruneThreshold caps limiter memory: when a state map grows past this many
// keys, stale entries are swept on the next access.
const pruneThreshold = 4096

// Bucket is a token-bucket limiter keyed by an arbitrary string (e.g.
// "ip|device").
type Bucket struct {
	mu       sync.Mutex
	capacity float64
	perSec   float64
	now      func() time.Time
	states   map[string]*bucketState
}

type bucketState struct {
	tokens float64
	last   time.Time
}

// NewBucket builds a token bucket allowing bursts of capacity requests,
// refilling at perSec tokens per second.
func NewBucket(capacity int, perSec float64, now func() time.Time) *Bucket {
	return &Bucket{
		capacity: float64(capacity),
		perSec:   perSec,
		now:      now,
		states:   map[string]*bucketState{},
	}
}

// Allow reports whether one request for key may proceed, consuming a token
// when it does.
func (b *Bucket) Allow(key string) bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	now := b.now()
	st, ok := b.states[key]
	if !ok {
		b.pruneLocked(now)
		st = &bucketState{tokens: b.capacity, last: now}
		b.states[key] = st
	}
	st.tokens += now.Sub(st.last).Seconds() * b.perSec
	if st.tokens > b.capacity {
		st.tokens = b.capacity
	}
	st.last = now
	if st.tokens < 1 {
		return false
	}
	st.tokens--
	return true
}

// pruneLocked drops entries that have refilled completely (idle long enough
// to be indistinguishable from fresh ones). Caller holds the lock.
func (b *Bucket) pruneLocked(now time.Time) {
	if len(b.states) < pruneThreshold {
		return
	}
	idle := time.Duration(b.capacity/b.perSec*float64(time.Second)) + time.Minute
	for key, st := range b.states {
		if now.Sub(st.last) > idle {
			delete(b.states, key)
		}
	}
}

// Backoff enforces per-key escalating delays (recovery GET: each
// attempt doubles the wait before the next is allowed, up to max; a quiet
// period of resetAfter clears the escalation).
type Backoff struct {
	mu         sync.Mutex
	base       time.Duration
	max        time.Duration
	resetAfter time.Duration
	now        func() time.Time
	states     map[string]*backoffState
}

type backoffState struct {
	nextAllowed time.Time
	delay       time.Duration
	last        time.Time
}

// NewBackoff builds an escalating-backoff limiter starting at base delay,
// doubling per attempt up to max, resetting after resetAfter of quiet.
func NewBackoff(base, max, resetAfter time.Duration, now func() time.Time) *Backoff {
	return &Backoff{base: base, max: max, resetAfter: resetAfter, now: now, states: map[string]*backoffState{}}
}

// Allow reports whether an attempt for key may proceed now. An allowed
// attempt arms the next (doubled) delay.
func (b *Backoff) Allow(key string) bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	now := b.now()
	st, ok := b.states[key]
	if ok && now.Sub(st.last) > b.resetAfter {
		delete(b.states, key)
		ok = false
	}
	if !ok {
		b.pruneLocked(now)
		b.states[key] = &backoffState{nextAllowed: now.Add(b.base), delay: b.base, last: now}
		return true
	}
	st.last = now
	if now.Before(st.nextAllowed) {
		return false
	}
	st.delay *= 2
	if st.delay > b.max {
		st.delay = b.max
	}
	st.nextAllowed = now.Add(st.delay)
	return true
}

// pruneLocked drops entries quiet past the reset window. Caller holds the
// lock.
func (b *Backoff) pruneLocked(now time.Time) {
	if len(b.states) < pruneThreshold {
		return
	}
	for key, st := range b.states {
		if now.Sub(st.last) > b.resetAfter {
			delete(b.states, key)
		}
	}
}

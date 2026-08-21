// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Vault-session state for the main window: the current lock status plus the
 * unlock/lock actions. Mounted once at the App root (like EspansoProvider); the
 * panel window does not use it.
 *
 * All crypto and the unlock state machine live in Rust — this only reflects
 * status and forwards actions. A background poll re-reads status so a session
 * that has passed its idle window is auto-locked host-side on the next tick
 * (the poll drives the server's idle-timeout enforcement).
 */
import { type VaultStatus, vaultLock, vaultStatus, vaultUnlockBiometric } from '@typvia/shared';
import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';

interface VaultContextValue {
  /** null until the first status read resolves. */
  status: VaultStatus | null;
  refresh: () => Promise<void>;
  /** Applies a freshly-read status (returned by an unlock/create call). */
  apply: (status: VaultStatus) => void;
  unlockBiometric: () => Promise<void>;
  lock: () => Promise<void>;
}

/**
 * No-op default so a page rendered outside the provider (e.g. in isolation
 * during tests) reads "unknown" and never drives a poll. The real App always
 * mounts {@link VaultProvider}.
 */
const DEFAULT: VaultContextValue = {
  status: null,
  refresh: async () => undefined,
  apply: () => undefined,
  unlockBiometric: async () => undefined,
  lock: async () => undefined,
};

const VaultContext = createContext<VaultContextValue>(DEFAULT);

/** Re-read status on this cadence so a stale unlocked session auto-locks. */
const POLL_INTERVAL_MS = 30_000;

export function VaultProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<VaultStatus | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await vaultStatus());
    } catch {
      // Status is best-effort; keep the prior value on a transient failure.
    }
  }, []);

  const apply = useCallback((next: VaultStatus) => setStatus(next), []);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  const unlockBiometric = useCallback(async () => {
    setStatus(await vaultUnlockBiometric());
  }, []);

  const lock = useCallback(async () => {
    setStatus(await vaultLock());
  }, []);

  return (
    <VaultContext.Provider value={{ status, refresh, apply, unlockBiometric, lock }}>
      {children}
    </VaultContext.Provider>
  );
}

export function useVault(): VaultContextValue {
  return useContext(VaultContext);
}

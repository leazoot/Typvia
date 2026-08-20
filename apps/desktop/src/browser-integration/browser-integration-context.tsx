/**
 * Browser-integration state for the main window: on/off status for the
 * Settings block plus a debounced "refresh the browser snapshot after a snippet
 * changed" poke. The host command no-ops while the integration is off, so
 * the poke fires unconditionally — no enabled gate on the caller side.
 */
import {
  browserIntegrationDisable,
  browserIntegrationEnable,
  browserIntegrationStatus,
  browserIntegrationSync,
  createSnapshotRefresher,
  type BrowserIntegrationStatus,
} from '@typvia/shared';
import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';

/**
 * App-wide debounced snapshot poke. Module-level (not React state) so the
 * espanso mutation-notify path can reach it without threading a second
 * context through every page; the same burst-coalescing refresher the
 * mobile keyboard snapshot uses.
 */
const refresher = createSnapshotRefresher(browserIntegrationSync);

/** Called after any snippet mutation; coalesces into one snapshot write. */
export function notifyBrowserIntegrationMutation(): void {
  refresher.notifyMutation();
}

interface BrowserIntegrationContextValue {
  status: BrowserIntegrationStatus | null;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
}

/**
 * No-op default so a page rendered outside the provider (tests, panel
 * window) reads "unknown" and never flips the switch. The real App always
 * mounts {@link BrowserIntegrationProvider}.
 */
const DEFAULT: BrowserIntegrationContextValue = {
  status: null,
  enable: async () => undefined,
  disable: async () => undefined,
};

const BrowserIntegrationContext = createContext<BrowserIntegrationContextValue>(DEFAULT);

export function BrowserIntegrationProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<BrowserIntegrationStatus | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await browserIntegrationStatus());
    } catch {
      // Status is best-effort; keep the prior value.
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const enable = useCallback(async () => {
    await browserIntegrationEnable();
    await refresh();
  }, [refresh]);

  const disable = useCallback(async () => {
    await browserIntegrationDisable();
    await refresh();
  }, [refresh]);

  return (
    <BrowserIntegrationContext.Provider value={{ status, enable, disable }}>
      {children}
    </BrowserIntegrationContext.Provider>
  );
}

export function useBrowserIntegration(): BrowserIntegrationContextValue {
  return useContext(BrowserIntegrationContext);
}

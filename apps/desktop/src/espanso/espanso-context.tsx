// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Espanso integration state for the main window: current status plus a
 * debounced "regenerate after a snippet changed" trigger. The panel window
 * does not mount this — only the main App does.
 *
 * Business rules stay in Rust: this only reads status and asks the host to
 * regenerate. Auto-sync is gated on `enabled` so a snippet edit never silently
 * creates the espanso config (turning the integration on is an explicit act).
 */
import {
  espansoCoexistence,
  espansoDisable,
  espansoStatus,
  espansoSync,
  type EspansoCoexistenceChoice,
  type EspansoStatus,
} from '@typvia/shared';

import { notifyBrowserIntegrationMutation } from '../browser-integration/browser-integration-context';
import { notifySemanticMutation } from '../semantic/semantic-poke';
import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';

interface EspansoContextValue {
  status: EspansoStatus | null;
  refresh: () => Promise<void>;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
  /** Records the coexistence answer (takeover / stand aside), then refreshes. */
  coexist: (choice: EspansoCoexistenceChoice) => Promise<void>;
  /** Called after a snippet mutation; regenerates the config when enabled. */
  notifyMutation: () => void;
}

/**
 * No-op default so a page rendered outside the provider (e.g. in isolation
 * during tests) reads "not configured" and never triggers a sync. The real App
 * always mounts {@link EspansoProvider}.
 */
const DEFAULT: EspansoContextValue = {
  status: null,
  refresh: async () => undefined,
  enable: async () => undefined,
  disable: async () => undefined,
  coexist: async () => undefined,
  notifyMutation: () => undefined,
};

const EspansoContext = createContext<EspansoContextValue>(DEFAULT);

/** Coalesce bursts of edits into one regeneration. */
const SYNC_DEBOUNCE_MS = 900;

export function EspansoProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<EspansoStatus | null>(null);
  const enabledRef = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await espansoStatus();
      enabledRef.current = next.enabled;
      setStatus(next);
    } catch {
      // Status is best-effort (espanso may be absent); keep the prior value.
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const enable = useCallback(async () => {
    await espansoSync();
    await refresh();
  }, [refresh]);

  const disable = useCallback(async () => {
    await espansoDisable();
    await refresh();
  }, [refresh]);

  const coexist = useCallback(
    async (choice: EspansoCoexistenceChoice) => {
      await espansoCoexistence(choice);
      await refresh();
    },
    [refresh],
  );

  const notifyMutation = useCallback(() => {
    // The browser snapshot and the embedding queue ride the same mutation
    // signal; their commands no-op while off, so no gate here.
    notifyBrowserIntegrationMutation();
    notifySemanticMutation();
    if (!enabledRef.current) return;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void espansoSync()
        .then(refresh)
        .catch(() => undefined);
    }, SYNC_DEBOUNCE_MS);
  }, [refresh]);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  return (
    <EspansoContext.Provider value={{ status, refresh, enable, disable, coexist, notifyMutation }}>
      {children}
    </EspansoContext.Provider>
  );
}

export function useEspanso(): EspansoContextValue {
  return useContext(EspansoContext);
}

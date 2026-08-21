// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The Sync overview state machine: what the account looks like right now and
 * the five things the user can do to it. Desktop draws it as a route, mobile
 * as a list — the fetching, the offline reading, the settling of the ambient
 * segment and every sentence are the same on both, so they live here once.
 */
import {
  type ConflictPair,
  type SyncDevice,
  type SyncStatus,
  syncConflicts,
  syncDevices,
  syncDisable,
  syncNow,
  syncResume,
  syncRevokeDevice,
  syncRotateKey,
  syncStatus,
} from '@typvia/shared';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { trFor, useLocale } from '../i18n';
import {
  offlineNotice,
  isOfflineError,
  messageOf,
  roundNotice,
  revokeNotice,
  rotateNotice,
  selfDevice,
} from './labels';

export interface SyncOverview {
  /** `null` until the first status answer — the loading state. */
  status: SyncStatus | null;
  devices: SyncDevice[];
  conflicts: ConflictPair[];
  /** The server could not be reached; the queue is intact. */
  offline: boolean;
  /** A round is in flight right now. */
  running: boolean;
  /** Keep the ambient segment painted until the current pass ends. */
  settling: boolean;
  /** A long-running action (revoke / resume / rotate) is in flight. */
  busy: boolean;
  notice: string | null;
  /** The device a revoke is being confirmed for, or `null`. */
  confirmRevoke: SyncDevice | null;
  askRevoke: (device: SyncDevice | null) => void;
  /** Timestamp the relative labels are rendered against; stable per mount. */
  now: number;
  refresh: () => Promise<void>;
  runSync: () => Promise<void>;
  revoke: (device: SyncDevice) => Promise<void>;
  resume: () => Promise<void>;
  /** Turns sync off, or back on for the already-bound account. */
  toggleEnabled: () => Promise<void>;
  rotate: () => Promise<void>;
  /** Called at the end of an ambient pass so a finished loop can stop. */
  endPass: () => void;
}

export function useSyncOverview(): SyncOverview {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [devices, setDevices] = useState<SyncDevice[]>([]);
  const [conflicts, setConflicts] = useState<ConflictPair[]>([]);
  const [offline, setOffline] = useState(false);
  const [running, setRunning] = useState(false);
  // The segment finishes its pass before it stops: `running` drives the loop,
  // `settling` keeps it painted until the current pass ends (design motion).
  const [settling, setSettling] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmRevoke, setConfirmRevoke] = useState<SyncDevice | null>(null);
  const now = useRef(Date.now()).current;
  const runningRef = useRef(false);

  const refresh = useCallback(async () => {
    const next = await syncStatus();
    setStatus(next);
    setConflicts(await syncConflicts());
    if (!next.configured || !next.enabled) {
      setDevices([selfDevice(next)]);
      return;
    }
    try {
      setDevices(await syncDevices());
      setOffline(false);
    } catch (caught) {
      // The line never disappears (design motion): a directory we could not
      // fetch leaves this device's own row standing, drawn as offline.
      setOffline(isOfflineError(caught));
      setDevices((current) => (current.length > 0 ? current : [selfDevice(next)]));
    }
  }, []);

  useEffect(() => {
    void refresh().catch(() => {
      setNotice(
        tr(
          'Sync state could not be read. Your snippets are unaffected.',
          '同步状态暂时读不到。你的片段不受影响。',
        ),
      );
    });
  }, [refresh, tr]);

  const runSync = useCallback(async () => {
    setRunning(true);
    runningRef.current = true;
    setSettling(true);
    setNotice(null);
    try {
      const round = await syncNow();
      setOffline(false);
      setNotice(roundNotice(round.pushed, round.applied, round.conflictCopies, locale));
    } catch (caught) {
      const wasOffline = isOfflineError(caught);
      setOffline(wasOffline);
      // No segment is painted while offline, so no pass will ever end: stop
      // the loop here instead of leaving it armed forever.
      setSettling(false);
      setNotice(wasOffline ? offlineNotice(locale) : messageOf(caught, locale));
    } finally {
      setRunning(false);
      runningRef.current = false;
      await refresh().catch(() => undefined);
    }
  }, [refresh, locale]);

  const revoke = useCallback(
    async (device: SyncDevice) => {
      setBusy(true);
      try {
        await syncRevokeDevice(device.deviceId);
        setConfirmRevoke(null);
        setNotice(revokeNotice(device.name, locale));
        await refresh();
      } catch (caught) {
        setNotice(messageOf(caught, locale));
      } finally {
        setBusy(false);
      }
    },
    [refresh, locale],
  );

  const resume = useCallback(async () => {
    setBusy(true);
    try {
      setStatus(await syncResume());
      await refresh();
    } catch (caught) {
      setNotice(messageOf(caught, locale));
    } finally {
      setBusy(false);
    }
  }, [refresh, locale]);

  const toggleEnabled = useCallback(async () => {
    if (status === null) return;
    setBusy(true);
    setNotice(null);
    try {
      setStatus(status.enabled ? await syncDisable() : await syncResume());
      await refresh();
    } catch (caught) {
      setNotice(messageOf(caught, locale));
    } finally {
      setBusy(false);
    }
  }, [status, refresh, locale]);

  const rotate = useCallback(async () => {
    setBusy(true);
    try {
      setNotice(rotateNotice(await syncRotateKey(), locale));
      await refresh();
    } catch (caught) {
      setNotice(messageOf(caught, locale));
    } finally {
      setBusy(false);
    }
  }, [refresh, locale]);

  const endPass = useCallback(() => {
    if (!runningRef.current) setSettling(false);
  }, []);

  return {
    status,
    devices,
    conflicts,
    offline,
    running,
    settling,
    busy,
    notice,
    confirmRevoke,
    askRevoke: setConfirmRevoke,
    now,
    refresh,
    runSync,
    revoke,
    resume,
    toggleEnabled,
    rotate,
    endPass,
  };
}

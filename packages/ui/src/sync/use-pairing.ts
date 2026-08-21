// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Both halves of pairing as headless state machines.
 *
 * Both sides stop at the same gate: the four groups of characters must match
 * on both screens before anything is installed or admitted. Neither hook
 * offers a way past it, and neither host may add one.
 */
import {
  ipcErrorCopy,
  pairingApprove,
  pairingBegin,
  pairingBeginWebdav,
  pairingCancel,
  type PairingClaim,
  pairingFinalize,
  pairingPoll,
  type PairingSas,
  pairingSas,
} from '@typvia/shared';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { trFor, useLocale } from '../i18n';
import { webdavCredentials } from './use-sync-setup';

/** How often the joining device asks whether the offer has arrived. */
const POLL_INTERVAL_MS = 2_000;

export interface PairAdmit {
  /** The code read from the joining device, as typed, scanned or pasted. */
  code: string;
  setCode: (code: string) => void;
  /** The verified offer awaiting the character comparison; `null` before it. */
  check: PairingSas | null;
  /** Whether the joining device would be given vault access. */
  allowVault: boolean;
  setAllowVault: (allow: boolean) => void;
  busy: boolean;
  error: string | null;
  /** Reads the code and computes the comparison. Admits nothing. */
  read: () => Promise<void>;
  /** Admits the device. Only reachable after the user confirmed the match. */
  approve: () => Promise<void>;
  /** "They do not match" — back to the code entry, nothing exchanged. */
  reject: () => void;
}

/**
 * This device already holds the library: read the other device's code, then
 * compare the four groups before admitting it.
 */
export function usePairAdmit(vaultReady: boolean, onDone: () => void): PairAdmit {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [code, setCode] = useState('');
  const [check, setCheck] = useState<PairingSas | null>(null);
  const [allowVault, setAllowVault] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const read = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setCheck(await pairingSas(code.trim()));
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('That code could not be read.', '这个配对码无法读取。'),
      );
    } finally {
      setBusy(false);
    }
  }, [code, tr]);

  const approve = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await pairingApprove(code.trim(), allowVault && vaultReady);
      onDone();
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('The other device was not admitted.', '另一台设备没有被接纳。'),
      );
    } finally {
      setBusy(false);
    }
  }, [code, allowVault, vaultReady, onDone, tr]);

  const reject = useCallback(() => {
    setCheck(null);
  }, []);

  return {
    code,
    setCode,
    check,
    allowVault,
    setAllowVault,
    busy,
    error,
    read,
    approve,
    reject,
  };
}

export interface PairJoin {
  /** Which backend the account lives on. */
  kind: 'server' | 'webdav';
  setKind: (kind: 'server' | 'webdav') => void;
  serverUrl: string;
  setServerUrl: (url: string) => void;
  accountId: string;
  setAccountId: (id: string) => void;
  /** WebDAV only: HTTP Basic credentials; both empty = anonymous. */
  username: string;
  setUsername: (value: string) => void;
  password: string;
  setPassword: (value: string) => void;
  /** The pairing code to show, once a session started. */
  code: string | null;
  /** The verified offer awaiting the character comparison. */
  claim: PairingClaim | null;
  masterPassword: string;
  setMasterPassword: (password: string) => void;
  busy: boolean;
  error: string | null;
  /** Whether both fields are filled in. */
  ready: boolean;
  /** Starts the session and produces the code. Installs nothing. */
  begin: () => Promise<void>;
  /** Installs the offer. Only reachable after the user confirmed the match. */
  finalize: () => Promise<void>;
  /** "They do not match" — abandons the session, nothing installed. */
  stop: () => void;
}

/**
 * This device is joining: show the code, wait for the offer, then compare the
 * four groups before installing anything.
 */
export function usePairJoin(onDone: () => void): PairJoin {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [kind, setKind] = useState<'server' | 'webdav'>('server');
  const [serverUrl, setServerUrl] = useState('');
  const [accountId, setAccountId] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [code, setCode] = useState<string | null>(null);
  const [claim, setClaim] = useState<PairingClaim | null>(null);
  const [masterPassword, setMasterPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const begin = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const started =
        kind === 'webdav'
          ? await pairingBeginWebdav(serverUrl.trim(), webdavCredentials(username, password))
          : await pairingBegin(serverUrl.trim(), accountId.trim());
      setCode(started.code);
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('The session could not be started.', '配对会话没能开始。'),
      );
    } finally {
      setBusy(false);
    }
  }, [kind, serverUrl, accountId, username, password, tr]);

  const poll = useCallback(async () => {
    try {
      const next = await pairingPoll();
      if (next !== null) setClaim(next);
    } catch {
      // A poll that fails is the other device not having answered yet or the
      // network being down; the flow keeps waiting and says so.
    }
  }, []);

  useEffect(() => {
    if (code === null || claim !== null) return undefined;
    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      clearInterval(timer);
    };
  }, [code, claim, poll]);

  const finalize = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await pairingFinalize(masterPassword === '' ? null : masterPassword);
      onDone();
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('Pairing could not be completed.', '配对未能完成。'),
      );
    } finally {
      setBusy(false);
    }
  }, [masterPassword, onDone, tr]);

  const stop = useCallback(() => {
    void pairingCancel();
    setClaim(null);
    setCode(null);
  }, []);

  return {
    kind,
    setKind,
    serverUrl,
    setServerUrl,
    accountId,
    setAccountId,
    username,
    setUsername,
    password,
    setPassword,
    code,
    claim,
    masterPassword,
    setMasterPassword,
    busy,
    error,
    ready: serverUrl.trim() !== '' && (kind === 'webdav' || accountId.trim() !== ''),
    begin,
    finalize,
    stop,
  };
}

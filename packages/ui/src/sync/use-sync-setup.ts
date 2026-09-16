// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * First-use state for sync: start an account on a server you run, or on a
 * WebDAV endpoint you own. Joining and recovering are navigations,
 * so they stay with the host that owns routing.
 */
import { ipcErrorCopy, syncEnable, syncEnableWebdav, webdavCredentials } from '@typvia/shared';
import { useCallback, useMemo, useState } from 'react';
import { trFor, useLocale } from '../i18n';

/** Which backend the account will be founded on. */
export type SyncBackendKind = 'server' | 'webdav';

export interface SyncSetupFlow {
  kind: SyncBackendKind;
  setKind: (kind: SyncBackendKind) => void;
  serverUrl: string;
  setServerUrl: (url: string) => void;
  /** WebDAV only: HTTP Basic credentials; both empty = anonymous endpoint. */
  username: string;
  setUsername: (value: string) => void;
  password: string;
  setPassword: (value: string) => void;
  busy: boolean;
  error: string | null;
  /** The address field holds something to try. */
  ready: boolean;
  start: () => Promise<void>;
}

export function useSyncSetup(onDone: () => void): SyncSetupFlow {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [kind, setKind] = useState<SyncBackendKind>('server');
  const [serverUrl, setServerUrl] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const start = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      if (kind === 'webdav') {
        await syncEnableWebdav(serverUrl.trim(), await webdavCredentials(username, password));
      } else {
        await syncEnable(serverUrl.trim());
      }
      onDone();
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('Could not reach that server.', '无法连接到那台服务器。'),
      );
    } finally {
      setBusy(false);
    }
  }, [kind, serverUrl, username, password, onDone, tr]);

  return {
    kind,
    setKind,
    serverUrl,
    setServerUrl,
    username,
    setUsername,
    password,
    setPassword,
    busy,
    error,
    ready: serverUrl.trim() !== '',
    start,
  };
}

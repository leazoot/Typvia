/**
 * Recovery code export and account recovery as headless machines.
 *
 * The export carries a correctness invariant that must not be re-derived per
 * host: the code is returned exactly once and stored nowhere, so the flow
 * cannot leave the screen until the user states they wrote it down.
 */
import {
  type RecoveryCode,
  type SyncStatus,
  recoveryPublish,
  recoveryRecover,
  recoveryRecoverWebdav,
  syncStatus,
} from '@typvia/shared';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { trFor, useLocale } from '../i18n';
import { webdavCredentials } from './use-sync-setup';

export interface RecoveryExport {
  /** `null` until the first status answer — the loading state. */
  status: SyncStatus | null;
  /** The generated code, shown exactly once. */
  code: RecoveryCode | null;
  /** The user stated they wrote it down; leaving is blocked until then. */
  saved: boolean;
  setSaved: (saved: boolean) => void;
  busy: boolean;
  error: string | null;
  /** True when a code can be made: a vault exists and is unlocked. */
  canPublish: boolean;
  publish: () => Promise<void>;
}

export function useRecoveryExport(): RecoveryExport {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [code, setCode] = useState<RecoveryCode | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void syncStatus()
      .then(setStatus)
      .catch(() => undefined);
  }, []);

  const publish = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setCode(await recoveryPublish());
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(
              `${caught.message} The previous recovery code, if any, still works.`,
              `${caught.message} 之前的恢复码（如果有）仍然有效。`,
            )
          : tr('The recovery code could not be created.', '恢复码没能生成。'),
      );
    } finally {
      setBusy(false);
    }
  }, [tr]);

  return {
    status,
    code,
    saved,
    setSaved,
    busy,
    error,
    canPublish: status !== null && status.vaultReady && status.vaultUnlocked,
    publish,
  };
}

export interface AccountRecover {
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
  code: string;
  setCode: (code: string) => void;
  masterPassword: string;
  setMasterPassword: (password: string) => void;
  busy: boolean;
  error: string | null;
  /** Every field is filled in. */
  ready: boolean;
  /** The account was taken over by this device. */
  done: boolean;
  recover: () => Promise<void>;
}

export function useAccountRecover(): AccountRecover {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [kind, setKind] = useState<'server' | 'webdav'>('server');
  const [serverUrl, setServerUrl] = useState('');
  const [accountId, setAccountId] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [code, setCode] = useState('');
  const [masterPassword, setMasterPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  const recover = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      if (kind === 'webdav') {
        await recoveryRecoverWebdav(
          serverUrl.trim(),
          webdavCredentials(username, password),
          code.trim(),
          masterPassword,
        );
      } else {
        await recoveryRecover(serverUrl.trim(), accountId.trim(), code.trim(), masterPassword);
      }
      setDone(true);
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(
              `${caught.message} Nothing on this device was changed.`,
              `${caught.message} 本机没有任何改动。`,
            )
          : tr('The account could not be recovered.', '账户未能恢复。'),
      );
    } finally {
      setBusy(false);
    }
  }, [kind, serverUrl, accountId, username, password, code, masterPassword, tr]);

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
    setCode,
    masterPassword,
    setMasterPassword,
    busy,
    error,
    ready:
      serverUrl.trim() !== '' &&
      (kind === 'webdav' || accountId.trim() !== '') &&
      code.trim() !== '' &&
      masterPassword !== '',
    done,
    recover,
  };
}

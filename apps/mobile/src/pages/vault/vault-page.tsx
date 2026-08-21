// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Mobile Vault: locked, unlocked and dark readings of one page.
 *
 * Three top-level states, chosen from the vault session status:
 *  - not initialized -> honest first-use setup (a page, never a modal)
 *  - initialized & locked -> the locked page: a centred ring, "Use Face ID"
 *    and the master-password path behind "Enter passcode"
 *  - unlocked -> masked rows revealed by press-and-hold (decrypt on demand,
 *    hidden again on release), the LOCKS IN countdown, and the New secret
 *    form.
 *
 * All crypto and the unlock state machine live in Rust; the biometric sheet
 * is presented by the OS itself while the host reads the gated key (the
 * userPresence Keychain item on iOS, the BiometricPrompt-bound Keystore
 * decrypt on Android). The design draws the iOS gate word ("Face ID"); the
 * platform layer substitutes Android's ("Fingerprint" / "指纹").
 * Unlock failures show one static line — never which part was wrong, never a
 * raw error message.
 */
import {
  type Folder,
  type Snippet,
  type VaultStatus,
  listFolderChildren,
  vaultCopySecret,
  vaultCreateSecret,
  vaultEnableBiometric,
  vaultInitialize,
  vaultList,
  vaultLock,
  vaultReset,
  vaultResetPreview,
  vaultReveal,
  vaultStatus,
  vaultUnlockBiometric,
  vaultUnlockPassword,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { biometricWord, biometricWordZh, isAndroid } from '../../platform';
import './vault.css';

/** A held reveal still auto-hides after this cap, even if never released. */
const REVEAL_TTL_MS = 8_000;

/** Metadata page size for the vault list (contents are never fetched). */
const LIST_LIMIT = 500;

/** Re-read status on this cadence so a stale unlocked session auto-locks
 * host-side on the next tick (mirrors the desktop provider poll). */
const POLL_INTERVAL_MS = 30_000;

/** Chinese reading of "Use <gate>": CJK gate words take no surrounding
 * spaces, Latin ones ("Face ID") keep them. */
const USE_GATE_ZH = isAndroid ? `使用${biometricWordZh}` : `使用 ${biometricWordZh}`;

export function VaultPage() {
  const tr = useTr();
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [statusFailed, setStatusFailed] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await vaultStatus());
      setStatusFailed(false);
    } catch {
      // Keep the prior status on a transient failure; first-load failures
      // get their own state below.
      setStatusFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  if (status === null) {
    if (statusFailed) {
      return (
        <main className="tv-mobile-page tv-mvault">
          <div className="tv-mobile-state">
            <div className="tv-mobile-state-title">
              {tr('Your secrets are safe on this device.', '你的秘密在本机安然无恙。')}
            </div>
            <div className="tv-mobile-state-text">
              {tr('The vault status could not be read just now.', '保险库状态暂时读不出来。')}
            </div>
            <button type="button" className="tv-mobile-state-action" onClick={() => void refresh()}>
              {tr('Retry', '重试')}
            </button>
          </div>
        </main>
      );
    }
    return (
      <main className="tv-mobile-page tv-mvault">
        <div className="tv-mvault-skeleton" aria-hidden="true" />
      </main>
    );
  }

  if (!status.initialized) {
    return <VaultFirstRun onInitialized={setStatus} />;
  }

  if (!status.unlocked) {
    return <VaultLocked onUnlocked={setStatus} />;
  }

  return <VaultUnlocked status={status} onStatus={setStatus} />;
}

// --- First use -------------------------------------------------------------

function VaultFirstRun({ onInitialized }: { onInitialized: (status: VaultStatus) => void }) {
  const tr = useTr();
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (busy) return;
      if (password.length < 8) {
        setError(
          tr('Choose a master password of at least 8 characters.', '主密码至少需要 8 个字符。'),
        );
        return;
      }
      if (password !== confirm) {
        setError(tr('The two passwords do not match.', '两次输入的密码不一致。'));
        return;
      }
      setBusy(true);
      setError(null);
      try {
        onInitialized(await vaultInitialize(password));
      } catch {
        setError(
          tr(
            'The vault could not be set up just now — nothing was saved.',
            '保险库暂时设置不了 —— 没有保存任何内容。',
          ),
        );
      } finally {
        setBusy(false);
        setPassword('');
        setConfirm('');
      }
    },
    [password, confirm, busy, onInitialized, tr],
  );

  return (
    <main className="tv-mobile-page tv-mvault">
      <div className="tv-mvault-center tv-mvault-setup">
        <div className="tv-mvault-ring" aria-hidden="true">
          <span className="tv-mvault-ring-caret" />
        </div>
        <h1 className="tv-mvault-lock-title">{tr('Set up the vault', '设置保险库')}</h1>
        <p className="tv-mvault-lock-sub">
          {tr('Contents decrypt only on this device', '内容仅在本机解密')}
        </p>
        <form className="tv-mvault-form" onSubmit={submit}>
          <input
            className="tv-mvault-field"
            type="password"
            aria-label={tr('Master password', '主密码')}
            placeholder={tr('Master password', '主密码')}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={busy}
          />
          <input
            className="tv-mvault-field"
            type="password"
            aria-label={tr('Confirm master password', '确认主密码')}
            placeholder={tr('Confirm password', '确认密码')}
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            disabled={busy}
          />
          <button type="submit" className="tv-mvault-gate" disabled={busy}>
            {tr('Create vault', '创建保险库')}
          </button>
        </form>
        {error !== null ? (
          <p className="tv-mvault-error" role="alert">
            {error}
          </p>
        ) : (
          <p className="tv-mvault-foot">
            {tr(
              `There is no recovery if you forget it — Typvia never sees your master password. You can enable ${biometricWord} after setup.`,
              isAndroid
                ? `忘记主密码将无法找回 —— Typvia 永远看不到它。设置完成后可以开启${biometricWordZh}解锁。`
                : `忘记主密码将无法找回 —— Typvia 永远看不到它。设置完成后可以开启 ${biometricWordZh} 解锁。`,
            )}
          </p>
        )}
      </div>
    </main>
  );
}

// --- Locked ----------------------------------------------------------------

function VaultLocked({ onUnlocked }: { onUnlocked: (status: VaultStatus) => void }) {
  const tr = useTr();
  const [mode, setMode] = useState<'choices' | 'password' | 'reset'>('choices');
  const [password, setPassword] = useState('');
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);

  const tryBiometric = useCallback(async () => {
    setBusy(true);
    setFailed(false);
    try {
      // The OS presents its biometric sheet during this call.
      onUnlocked(await vaultUnlockBiometric());
    } catch {
      // Not enrolled, cancelled, or failed — fall through to the password.
      setFailed(true);
      setMode('password');
    } finally {
      setBusy(false);
    }
  }, [onUnlocked]);

  const submitPassword = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (password === '' || busy) return;
      setBusy(true);
      setFailed(false);
      try {
        const status = await vaultUnlockPassword(password);
        setPassword('');
        onUnlocked(status);
      } catch {
        setFailed(true);
      } finally {
        setBusy(false);
      }
    },
    [password, busy, onUnlocked],
  );

  return (
    <main className="tv-mobile-page tv-mvault">
      {/* Generic placeholder lines behind the lock — never real titles or
          values, so nothing sensitive exists to blur. */}
      <div className="tv-mvault-veil" aria-hidden="true">
        <div className="tv-mvault-veil-item">
          <span className="tv-mvault-veil-title" />
          <span className="tv-mvault-veil-line" />
        </div>
        <div className="tv-mvault-veil-item">
          <span className="tv-mvault-veil-title" />
          <span className="tv-mvault-veil-line" />
        </div>
      </div>

      <div className="tv-mvault-center">
        <div className="tv-mvault-ring" aria-hidden="true">
          <span className="tv-mvault-ring-caret" />
        </div>
        <h1 className="tv-mvault-lock-title">{tr('Vault', '保险库')}</h1>
        <p className="tv-mvault-lock-sub">{tr('Protected on this device', '仅在本机受保护')}</p>
      </div>

      <div className="tv-mvault-lock-foot">
        {mode === 'reset' ? (
          <ResetFlow onBack={() => setMode('password')} onReset={onUnlocked} />
        ) : mode === 'password' ? (
          <>
            <form className="tv-mvault-form tv-mvault-form-center" onSubmit={submitPassword}>
              <input
                className="tv-mvault-field tv-mvault-field-center"
                type="password"
                aria-label={tr('Master password', '主密码')}
                placeholder={tr('Master password', '主密码')}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                disabled={busy}
              />
              <button type="submit" className="tv-mvault-gate" disabled={busy || password === ''}>
                {tr('Unlock', '解锁')}
              </button>
            </form>
            {/* Failure states name what is still good before asking for
                action; the line is static and never says which part failed. */}
            {failed ? (
              <p className="tv-mvault-error" role="alert">
                {tr(
                  'Still encrypted and safe — that attempt didn’t unlock.',
                  '内容仍然加密、仍然安全 —— 这次尝试没有解锁。',
                )}
              </p>
            ) : null}
            <button
              type="button"
              className="tv-mvault-forgot"
              onClick={() => setMode('reset')}
              disabled={busy}
            >
              {tr('Forgot the master password?', '忘记主密码？')}
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className="tv-mvault-gate"
              onClick={() => void tryBiometric()}
              disabled={busy}
            >
              {tr(`Use ${biometricWord}`, USE_GATE_ZH)}
            </button>
            <button
              type="button"
              className="tv-mvault-pass-link"
              onClick={() => setMode('password')}
              disabled={busy}
            >
              {tr('Enter passcode', '输入主密码')}
            </button>
          </>
        )}
      </div>
    </main>
  );
}

/**
 * The lost-password exit. There is no recovery by design, so the
 * only path forward is destruction: the flow names the real count, confirms
 * in two steps, and never uses a filled red button.
 */
function ResetFlow({
  onBack,
  onReset,
}: {
  onBack: () => void;
  onReset: (status: VaultStatus) => void;
}) {
  const tr = useTr();
  const [total, setTotal] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    // The count includes trashed secrets — the confirm copy must be the
    // whole truth. A failed preview keeps the flow usable with plain words.
    vaultResetPreview()
      .then(setTotal)
      .catch(() => setTotal(null));
  }, []);

  const destroyLabel =
    total === null
      ? tr('Destroy all secret snippets forever', '永久销毁全部密钥片段')
      : tr(
          total === 1
            ? 'Destroy 1 secret snippet forever'
            : `Destroy ${String(total)} secret snippets forever`,
          `永久销毁 ${String(total)} 个密钥片段`,
        );

  const run = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    setFailed(false);
    try {
      onReset(await vaultReset());
    } catch {
      setFailed(true);
      setBusy(false);
    }
  }, [busy, onReset]);

  return (
    <div className="tv-mvault-reset">
      <p className="tv-mvault-reset-warn" role="alert">
        {tr(
          'The master password cannot be recovered — Typvia never sees it. Resetting destroys the vault key and every secret snippet, here and on every synced device. This cannot be undone.',
          '主密码无法找回 —— Typvia 从未见过它。重置将销毁保险库密钥与全部密钥片段,并在所有同步设备上一并删除,无法撤销。',
        )}
      </p>
      <div className="tv-mvault-reset-actions">
        <button type="button" className="tv-mvault-pass-link" onClick={onBack} disabled={busy}>
          {tr('Back to unlock', '返回解锁')}
        </button>
        {confirming ? (
          <button
            type="button"
            className="tv-mvault-danger"
            onClick={() => void run()}
            disabled={busy}
          >
            {busy ? tr('Resetting…', '重置中…') : destroyLabel}
          </button>
        ) : (
          <button
            type="button"
            className="tv-mvault-danger"
            onClick={() => setConfirming(true)}
            disabled={busy}
          >
            {tr('Reset the vault', '重置保险库')}
          </button>
        )}
      </div>
      {failed && (
        <p className="tv-mvault-error" role="alert">
          {tr(
            'Nothing was changed — the reset didn’t complete. Try again.',
            '保险库未被更改 —— 这次重置没有完成,请重试。',
          )}
        </p>
      )}
    </div>
  );
}

// --- Unlocked --------------------------------------------------------------

function VaultUnlocked({
  status,
  onStatus,
}: {
  status: VaultStatus;
  onStatus: (status: VaultStatus) => void;
}) {
  const tr = useTr();
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [creating, setCreating] = useState(false);
  // Top-level folder names for the row meta line; a failed read simply
  // leaves the folder part off (the design omits it when there is none).
  const [folders, setFolders] = useState<ReadonlyMap<string, string>>(new Map());

  const load = useCallback(async () => {
    try {
      setRows(await vaultList(LIST_LIMIT, 0));
      setFailed(false);
    } catch {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    listFolderChildren(null)
      .then((list: Folder[]) => setFolders(new Map(list.map((f) => [f.id, f.name]))))
      .catch(() => undefined);
  }, []);

  const lockNow = useCallback(async () => {
    try {
      onStatus(await vaultLock());
    } catch {
      // Locking failed host-side; the idle timeout still stands.
    }
  }, [onStatus]);

  return (
    <main className="tv-mobile-page tv-mvault">
      <header className="tv-mvault-head">
        <div className="tv-mvault-head-brand">
          <span className="tv-mvault-head-bar" aria-hidden="true" />
          <h1 className="tv-mvault-title">{tr('Vault', '保险库')}</h1>
        </div>
        <RelockCountdown status={status} />
      </header>

      <div className="tv-mvault-toolbar">
        <button type="button" className="tv-mvault-tool" onClick={() => void lockNow()}>
          {tr('Lock now', '立即锁定')}
        </button>
        <button type="button" className="tv-mvault-tool" onClick={() => setCreating((v) => !v)}>
          {tr('New secret', '新建秘密')}
        </button>
      </div>

      {creating && (
        <NewSecretForm
          onCancel={() => setCreating(false)}
          onCreated={() => {
            setCreating(false);
            void load();
          }}
        />
      )}

      <VaultRows rows={rows} failed={failed} folders={folders} />

      <div className="tv-mvault-hint">
        <span className="tv-mvault-hint-bar" aria-hidden="true" />
        <span className="tv-mvault-hint-text">
          {tr('Hold to reveal · nothing is copied to the clipboard', '按住查看 · 不会复制到剪贴板')}
        </span>
      </div>

      <SecureFieldExplainer />

      <p className="tv-mvault-foot">
        {tr(
          'Contents are encrypted on this device and never indexed. Nothing here is ever sent to AI.',
          '内容在本机加密,从不建立索引。这里的任何内容都不会发送给 AI。',
        )}
      </p>
      <BiometricEnroll />
    </main>
  );
}

/** The auto-lock reading ("LOCKS IN 2:00"), ticking every second
 * from the host's idle timeout and last activity. */
function RelockCountdown({ status }: { status: VaultStatus }) {
  const tr = useTr();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(timer);
  }, []);
  const base = status.lastActivityAt ?? status.unlockedAt;
  if (base === null) return null;
  const remaining = Math.max(0, base + status.idleTimeoutMs - now);
  const mm = String(Math.floor(remaining / 60_000));
  const ss = Math.floor((remaining % 60_000) / 1_000)
    .toString()
    .padStart(2, '0');
  return (
    <span className="tv-mvault-countdown">{tr(`LOCKS IN ${mm}:${ss}`, `${mm}:${ss} 后锁定`)}</span>
  );
}

function BiometricEnroll() {
  const tr = useTr();
  const [state, setState] = useState<'idle' | 'done' | 'error'>('idle');
  const enroll = useCallback(async () => {
    try {
      await vaultEnableBiometric();
      setState('done');
    } catch {
      // Unavailable backend (no Face ID on this host) — the password path stays.
      setState('error');
    }
  }, []);
  if (state === 'done') {
    return (
      <p className="tv-mvault-enroll-note">
        {/* CJK gate words take no surrounding spaces; Latin ones keep them. */}
        {tr(
          `${biometricWord} is on for this vault.`,
          isAndroid ? `已开启${biometricWordZh}解锁。` : `已开启 ${biometricWordZh} 解锁。`,
        )}
      </p>
    );
  }
  return (
    <div className="tv-mvault-enroll">
      <button type="button" className="tv-mvault-pass-link" onClick={() => void enroll()}>
        {tr(
          `Enable ${biometricWord} for faster unlock`,
          isAndroid ? `开启${biometricWordZh}以更快解锁` : `开启 ${biometricWordZh} 以更快解锁`,
        )}
      </button>
      {state === 'error' && (
        <span className="tv-mvault-enroll-err">
          {tr(
            `${biometricWord} isn’t available here — the master password still works.`,
            isAndroid
              ? `${biometricWordZh}在这里不可用 —— 主密码仍然有效。`
              : `${biometricWordZh} 在这里不可用 —— 主密码仍然有效。`,
          )}
        </span>
      )}
    </div>
  );
}

function VaultRows({
  rows,
  failed,
  folders,
}: {
  rows: Snippet[] | null;
  failed: boolean;
  folders: ReadonlyMap<string, string>;
}) {
  const tr = useTr();
  if (failed) {
    return (
      <p className="tv-mvault-empty">
        {tr(
          'Your secrets are safe and encrypted on this device — the list couldn’t load just now.',
          '你的秘密在本机安然加密 —— 只是列表暂时加载不出来。稍后再试。',
        )}
      </p>
    );
  }
  if (rows === null) {
    return <div className="tv-mvault-skeleton" aria-hidden="true" />;
  }
  if (rows.length === 0) {
    return (
      <p className="tv-mvault-empty">
        {tr(
          'Nothing in the vault yet. Add a secret with New secret above.',
          '保险库还是空的。用上面的「新建秘密」添加一条。',
        )}
      </p>
    );
  }
  return (
    <ul className="tv-mvault-list">
      {rows.map((row) => (
        <SecretRow
          key={row.id}
          row={row}
          folderName={row.folderId === null ? null : (folders.get(row.folderId) ?? null)}
        />
      ))}
    </ul>
  );
}

function SecretRow({ row, folderName }: { row: Snippet; folderName: string | null }) {
  const tr = useTr();
  const [revealed, setRevealed] = useState<string | null>(null);
  const [error, setError] = useState(false);
  /** True while the press is held; a decrypt that lands after release is
   * dropped so the plaintext never appears once the finger lifted. */
  const holdingRef = useRef(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** Live copy countdown; total comes from the host so 30s is never duplicated. */
  const [countdown, setCountdown] = useState<{ remainingMs: number; totalMs: number } | null>(null);
  const [copyFailed, setCopyFailed] = useState(false);
  const copyTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearReveal = useCallback(() => {
    if (hideTimer.current) clearTimeout(hideTimer.current);
    setRevealed(null);
  }, []);

  useEffect(
    () => () => {
      clearReveal();
      if (copyTimer.current) clearInterval(copyTimer.current);
    },
    [clearReveal],
  );

  const beginReveal = useCallback(() => {
    holdingRef.current = true;
    setError(false);
    // One row, on demand — the list itself never carries plaintext.
    vaultReveal(row.id)
      .then((text) => {
        if (!holdingRef.current) return;
        setRevealed(text);
        if (hideTimer.current) clearTimeout(hideTimer.current);
        hideTimer.current = setTimeout(() => setRevealed(null), REVEAL_TTL_MS);
      })
      .catch(() => setError(true));
  }, [row.id]);

  const endReveal = useCallback(() => {
    holdingRef.current = false;
    clearReveal();
  }, [clearReveal]);

  const copy = useCallback(async () => {
    setCopyFailed(false);
    try {
      // The host puts the plaintext on the system pasteboard with an
      // OS-enforced expiry; only the clear delay comes back over IPC.
      const totalMs = await vaultCopySecret(row.id);
      const endsAt = Date.now() + totalMs;
      setCountdown({ remainingMs: totalMs, totalMs });
      if (copyTimer.current) clearInterval(copyTimer.current);
      copyTimer.current = setInterval(() => {
        const remainingMs = endsAt - Date.now();
        if (remainingMs <= 0) {
          // The OS enforced the clear — the row simply returns to rest.
          if (copyTimer.current) clearInterval(copyTimer.current);
          copyTimer.current = null;
          setCountdown(null);
        } else {
          setCountdown({ remainingMs, totalMs });
        }
      }, 1_000);
    } catch {
      setCopyFailed(true);
    }
  }, [row.id]);

  return (
    <li className="tv-mvault-row">
      <button
        type="button"
        className="tv-mvault-hold"
        aria-label={tr(`Hold to reveal ${row.title}`, `按住查看${row.title}`)}
        onPointerDown={beginReveal}
        onPointerUp={endReveal}
        onPointerLeave={endReveal}
        onPointerCancel={endReveal}
        onKeyDown={(event) => {
          if ((event.key === 'Enter' || event.key === ' ') && !event.repeat) beginReveal();
        }}
        onKeyUp={(event) => {
          if (event.key === 'Enter' || event.key === ' ') endReveal();
        }}
      >
        <span className="tv-mvault-row-title">{row.title}</span>
        {revealed !== null ? (
          <code className="tv-mvault-row-plain">{revealed}</code>
        ) : (
          <span className="tv-mvault-row-mask" aria-hidden="true">
            ••••••••••••
          </span>
        )}
        <span className="tv-mvault-row-meta">
          {tr('SECRET', '密钥')}
          {folderName !== null && ` · ${folderName}`}
        </span>
      </button>
      <span className="tv-mvault-row-side">
        {countdown !== null ? (
          <span className="tv-mvault-copyclear" role="status">
            <span className="tv-mvault-copyclear-bar" aria-hidden="true">
              <span
                className="tv-mvault-copyclear-fill"
                style={{
                  transform: `scaleX(${String(countdown.remainingMs / countdown.totalMs)})`,
                }}
              />
            </span>
            <span className="tv-mvault-copyclear-text">
              {tr(
                `clears in ${String(Math.ceil(countdown.remainingMs / 1_000))}s`,
                `${String(Math.ceil(countdown.remainingMs / 1_000))} 秒后清除`,
              )}
            </span>
          </span>
        ) : (
          <button type="button" className="tv-mvault-copy" onClick={() => void copy()}>
            {tr('Copy 30s', '复制 30 秒')}
          </button>
        )}
        {error && <span className="tv-mvault-row-err">{tr('try again', '请重试')}</span>}
        {copyFailed && (
          <span className="tv-mvault-row-copyerr" role="alert">
            {tr(
              'Your secret is still encrypted — the copy didn’t happen.',
              '你的秘密仍然加密 —— 这次复制没有发生。',
            )}
          </span>
        )}
      </span>
    </li>
  );
}

/**
 * The honest-downgrade explainer: password fields never get secrets typed by
 * a keyboard — iOS hides every third-party keyboard there, and on Android
 * vault items never enter Typvia's keyboard (and it refuses password fields
 * outright) — so the one-time copy round trip is stated here, on the vault
 * page, as a collapsible plain block: the real cause is named and the
 * clipboard policy is on screen before any copy happens. The design draws
 * only the iOS reading; the Android lead keeps the same honest frame with
 * Android's cause and gate word. The design's Android direct-injection
 * narrative describes a future IME feature, not this copy round trip.
 */
function SecureFieldExplainer() {
  const tr = useTr();
  return (
    <details className="tv-mvault-securefield">
      <summary className="tv-mvault-securefield-summary">
        {tr(
          'Secure field — the honest downgrade, in three steps',
          '安全输入框 —— 诚实的降级,三步完成',
        )}
      </summary>
      <div className="tv-mvault-securefield-body">
        <p className="tv-mvault-securefield-lead">
          {isAndroid
            ? tr(
                'Typvia’s keyboard never carries vault secrets and won’t type into password fields. Use the three steps below — the clipboard clears itself after 30 seconds.',
                '保险库内容从不进入 Typvia 的键盘,键盘也不会在密码框里输入。用下面三步即可 —— 剪贴板会在 30 秒后自动清空。',
              )
            : tr(
                'iOS won’t let any third-party keyboard type here. That is a system rule, not a Typvia choice. Use the three steps below — the clipboard clears itself after 30 seconds.',
                'iOS 不允许任何第三方键盘在这里输入。这是系统限制,不是 Typvia 的问题。用下面三步即可 —— 剪贴板会在 30 秒后自动清空。',
              )}
        </p>
        <ol className="tv-mvault-securefield-steps">
          <li>{tr(`Open Typvia · ${biometricWord}`, `打开 Typvia · ${biometricWordZh}`)}</li>
          <li>{tr('Copy once — clears in 30s', '复制一次 —— 30 秒后清除')}</li>
          <li>{tr('Come back and paste', '回来粘贴')}</li>
        </ol>
      </div>
    </details>
  );
}

function NewSecretForm({ onCancel, onCreated }: { onCancel: () => void; onCreated: () => void }) {
  const tr = useTr();
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (busy) return;
      if (title.trim() === '' || body === '') {
        setError(tr('A title and a secret value are both required.', '标题和秘密内容都是必填的。'));
        return;
      }
      setBusy(true);
      setError(null);
      try {
        await vaultCreateSecret({
          title: title.trim(),
          body,
          snippetType: 'sensitive',
          description: null,
          folderId: null,
          trigger: null,
          triggerMode: null,
          language: null,
        });
        setTitle('');
        setBody('');
        onCreated();
      } catch {
        setError(
          tr(
            'The secret could not be saved just now — nothing was stored.',
            '这条秘密暂时保存不了 —— 没有存储任何内容。',
          ),
        );
      } finally {
        setBusy(false);
      }
    },
    [title, body, busy, onCreated, tr],
  );

  return (
    <form className="tv-mvault-form tv-mvault-new" onSubmit={submit}>
      <input
        className="tv-mvault-field"
        type="text"
        aria-label={tr('Secret title', '秘密标题')}
        placeholder={tr('Title', '标题')}
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        disabled={busy}
      />
      <textarea
        className="tv-mvault-textarea"
        aria-label={tr('Secret value', '秘密内容')}
        placeholder={tr(
          'The secret value — encrypted before it is saved',
          '秘密内容 —— 保存前先加密',
        )}
        value={body}
        onChange={(e) => setBody(e.target.value)}
        disabled={busy}
        rows={3}
      />
      <div className="tv-mvault-new-actions">
        <button
          type="button"
          className="tv-mvault-pass-link"
          onClick={() => {
            setBody('');
            onCancel();
          }}
          disabled={busy}
        >
          {tr('Cancel', '取消')}
        </button>
        <button type="submit" className="tv-mvault-gate" disabled={busy}>
          {tr('Save secret', '保存秘密')}
        </button>
      </div>
      {error !== null && (
        <p className="tv-mvault-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

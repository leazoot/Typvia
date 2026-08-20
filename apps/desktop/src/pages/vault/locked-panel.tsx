/**
 * The locked vault, in place. Locking no longer throws a modal
 * over the app: the page keeps its header and its shape, the list becomes a
 * ghost of itself, and the two unlock paths sit where the list was.
 *
 * The failure copy never says which part of an attempt was wrong.
 */
import {
  type VaultStatus,
  vaultReset,
  vaultResetPreview,
  vaultUnlockPassword,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Action } from '../../workspace/kit';

interface LockedPanelProps {
  /** Secrets waiting behind the lock — metadata only, nothing is decrypted. */
  itemCount: number;
  onUnlocked: (status: VaultStatus) => void;
  onUnlockBiometric: () => Promise<void>;
  /** Called after a completed reset with the fresh uninitialized status. */
  onReset: (status: VaultStatus) => void;
}

export function LockedPanel({
  itemCount,
  onUnlocked,
  onUnlockBiometric,
  onReset,
}: LockedPanelProps) {
  const tr = useTr();
  const [mode, setMode] = useState<'choices' | 'password' | 'reset'>('choices');
  const [password, setPassword] = useState('');
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const passwordRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (mode === 'password') passwordRef.current?.focus();
  }, [mode]);

  const tryBiometric = useCallback(async () => {
    setBusy(true);
    setFailed(false);
    try {
      await onUnlockBiometric();
    } catch {
      // Not enrolled, cancelled, or failed — fall through to the password.
      setFailed(true);
      setMode('password');
    } finally {
      setBusy(false);
    }
  }, [onUnlockBiometric]);

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

  if (mode === 'reset') {
    return <ResetFlow onBack={() => setMode('password')} onReset={onReset} />;
  }

  return (
    <div className="tvv-locked">
      <div className="tvv-locked-word">{tr('Locked', '已锁定')}</div>
      <p className="tvv-locked-sub">
        {failed
          ? tr(
              'Still encrypted and safe — that attempt didn’t unlock.',
              '内容仍安全地加密着——这次尝试未能解锁。',
            )
          : tr(
              itemCount === 1
                ? '1 secret is kept safe on this Mac.'
                : `${String(itemCount)} secrets are kept safe on this Mac.`,
              `${String(itemCount)} 条密文安全保存在本机。`,
            )}
      </p>

      {mode === 'password' ? (
        <form className="tvv-locked-form" onSubmit={submitPassword}>
          <input
            ref={passwordRef}
            type="password"
            aria-label={tr('Master password', '主密码')}
            placeholder={tr('Master password', '主密码')}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            disabled={busy}
          />
          <button type="submit" className="tvw-chip" disabled={busy || password === ''}>
            {tr('Unlock', '解锁')}
          </button>
        </form>
      ) : (
        <div className="tvv-locked-actions">
          <button
            type="button"
            className="tvw-chip"
            disabled={busy}
            onClick={() => void tryBiometric()}
          >
            {tr('Unlock with Touch ID', '使用 Touch ID 解锁')}
          </button>
          <Action label={tr('Master password', '主密码')} onRun={() => setMode('password')} />
        </div>
      )}

      <div className="tvv-ghost" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>

      <div style={{ marginTop: 22 }}>
        <button type="button" className="tvv-danger" onClick={() => setMode('reset')}>
          {tr('Forgot the master password?', '忘记主密码?')}
        </button>
      </div>
    </div>
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
  const [count, setCount] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    // The count includes trashed secrets — the confirm copy must be the whole
    // truth. A failed preview keeps the flow usable with plain words.
    vaultResetPreview()
      .then(setCount)
      .catch(() => setCount(null));
  }, []);

  const destroyLabel =
    count === null
      ? tr('Destroy all secret snippets forever', '永久销毁全部密钥片段')
      : tr(
          count === 1
            ? 'Destroy 1 secret snippet forever'
            : `Destroy ${String(count)} secret snippets forever`,
          `永久销毁 ${String(count)} 个密钥片段`,
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
    <div className="tvv-locked">
      <div className="tvv-locked-word">{tr('Reset the vault', '重置保险库')}</div>
      <p className="tvv-reset">
        {tr(
          'The master password cannot be recovered — Typvia never sees it. Resetting destroys the vault key and every secret snippet, here and on every synced device. This cannot be undone.',
          '主密码无法找回——Typvia 从未见过它。重置将销毁保险库密钥与全部密钥片段,并在所有同步设备上一并删除,无法撤销。',
        )}
      </p>
      <div className="tvv-reset-actions">
        <Action label={tr('Back to unlock', '返回解锁')} onRun={onBack} />
        <button
          type="button"
          className="tvv-danger"
          disabled={busy}
          onClick={() => (confirming ? void run() : setConfirming(true))}
        >
          {confirming ? destroyLabel : tr('Reset the vault', '重置保险库')}
        </button>
      </div>
      {failed && (
        <p className="tvv-error" role="alert">
          {tr(
            'Nothing was changed — the reset didn’t complete. Try again.',
            '保险库未被更改——这次重置未能完成,请重试。',
          )}
        </p>
      )}
    </div>
  );
}

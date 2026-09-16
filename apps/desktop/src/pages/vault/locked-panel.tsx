// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The locked vault, in place: how many are behind the lock, one line for the
 * master password with the caret already in it, and Touch ID as words. The
 * failure copy never says which part of an attempt was wrong.
 */
import {
  type VaultStatus,
  vaultList,
  vaultReset,
  vaultResetPreview,
  vaultUnlockPassword,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState, type FormEvent } from 'react';
import { TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';

export function LockedPanel({
  status,
  onUnlocked,
  onUnlockBiometric,
  onReset,
}: {
  status: VaultStatus;
  onUnlocked: (status: VaultStatus) => void;
  onUnlockBiometric: () => Promise<void>;
  /** Called after a completed reset with the fresh, not-set-up status. */
  onReset: (status: VaultStatus) => void;
}) {
  const tr = useTr();
  const [count, setCount] = useState<number | null>(null);
  const [password, setPassword] = useState('');
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [resetting, setResetting] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Metadata only — nothing is decrypted to know how many are in there.
    vaultList(500, 0)
      .then((rows) => setCount(rows.length))
      .catch(() => setCount(null));
  }, []);
  useEffect(() => {
    if (!resetting) input.current?.focus();
  }, [resetting]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (password === '' || busy) return;
    setBusy(true);
    setFailed(false);
    try {
      onUnlocked(await vaultUnlockPassword(password));
    } catch {
      setFailed(true);
    } finally {
      setPassword('');
      setBusy(false);
    }
  };

  const touchId = async () => {
    setBusy(true);
    setFailed(false);
    try {
      await onUnlockBiometric();
    } catch {
      // Not enrolled, cancelled or refused: the password line is right there.
      setFailed(true);
      input.current?.focus();
    } finally {
      setBusy(false);
    }
  };

  if (resetting) return <ResetFlow onBack={() => setResetting(false)} onReset={onReset} />;

  const n = count === null ? null : String(count);
  const minutes = String(Math.max(1, Math.round(status.idleTimeoutMs / 60_000)));
  return (
    <section className="tvv-card" aria-labelledby="tvv-locked-title">
      <div className="tvv-card-head">
        <Mascot state="locked" size={32} />
        <span className="tpi-eyebrow">{tr('Vault · locked', '保险库 · 已锁')}</span>
      </div>
      <h1 id="tvv-locked-title" className="tvv-lead">
        {n === null
          ? tr('Your secrets are locked in here.', '你的密文锁在这里。')
          : tr(
              count === 1 ? '1 snippet is locked in here.' : `${n} snippets are locked in here.`,
              `${n} 条片段锁在这里。`,
            )}
      </h1>
      <form onSubmit={(event) => void submit(event)}>
        <span className="tvv-line is-secret">
          <input
            ref={input}
            type="password"
            autoComplete="current-password"
            aria-label={tr('Master password', '主密码')}
            value={password}
            disabled={busy}
            onChange={(event) => setPassword(event.target.value)}
          />
        </span>
      </form>
      <p className="tvv-hint" role={failed ? 'alert' : undefined}>
        {failed
          ? tr(
              'Still encrypted and safe — that attempt did not unlock it.',
              '内容仍安全地加密着 —— 这次没有解开。',
            )
          : tr(
              `Master password or Touch ID. It locks itself again after ${minutes} minutes idle.`,
              `主密码或 Touch ID。解锁后闲置 ${minutes} 分钟自动重锁。`,
            )}
      </p>
      <div className="tvv-card-foot">
        <TextAction primary onClick={() => void touchId()}>
          {tr('Unlock with Touch ID', '用 Touch ID 解锁')}
        </TextAction>
        <span className="tvv-grow" />
        <button type="button" className="tvv-quiet" onClick={() => setResetting(true)}>
          {tr('Forgot the master password?', '忘了主密码?')}
        </button>
      </div>
    </section>
  );
}

/**
 * The lost-password exit. There is no recovery by design, so the only way on
 * is destruction: it names the real count and takes two presses.
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
    // The count includes deleted secrets — the question must be the whole truth.
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

  const run = async () => {
    if (busy) return;
    setBusy(true);
    setFailed(false);
    try {
      onReset(await vaultReset());
    } catch {
      setFailed(true);
      setBusy(false);
    }
  };

  return (
    <section className="tvv-card" aria-labelledby="tvv-reset-title">
      <div className="tvv-card-head">
        <Mascot state="confused" size={32} />
        <span className="tpi-eyebrow">{tr('Vault · reset', '保险库 · 重置')}</span>
      </div>
      <h1 id="tvv-reset-title" className="tvv-lead">
        {tr('Forgot the master password', '忘了主密码')}
      </h1>
      <p className="tvv-text">
        {tr(
          'The master password cannot be recovered — Typvia never sees it. Resetting destroys the vault key and every secret snippet, here and on every synced device. This cannot be undone.',
          '主密码无法找回——Typvia 从未见过它。重置将销毁保险库密钥与全部密钥片段,并在所有同步设备上一并删除,无法撤销。',
        )}
      </p>
      {failed && (
        <p className="tvv-hint" role="alert">
          {tr(
            'Nothing was changed — the reset did not complete. Try again.',
            '保险库没有变 —— 这次重置没做完,可以再试一次。',
          )}
        </p>
      )}
      <div className="tvv-card-foot">
        <button
          type="button"
          className="tvv-destroy"
          disabled={busy}
          onClick={() => (confirming ? void run() : setConfirming(true))}
        >
          {confirming ? destroyLabel : tr('Reset the vault', '重置保险库')}
        </button>
        <span className="tvv-grow" />
        <button type="button" className="tvv-quiet" onClick={onBack}>
          {tr('Back to unlock', '返回解锁')}
        </button>
      </div>
    </section>
  );
}

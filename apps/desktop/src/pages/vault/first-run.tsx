// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  IpcError,
  ipcErrorCopy,
  masterPasswordMinLength,
  type VaultStatus,
  vaultInitialize,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState, type FormEvent } from 'react';
import { TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';

/** Setting the vault up: one master password, typed twice, on two lines. */
export function VaultFirstRun({ onInitialized }: { onInitialized: (status: VaultStatus) => void }) {
  const tr = useTr();
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The floor is the vault's. This page only says it out loud, and until it
  // has been told, it does not guess.
  const [minimum, setMinimum] = useState<number | null>(null);

  useEffect(() => {
    masterPasswordMinLength()
      .then(setMinimum)
      .catch(() => setMinimum(null));
  }, []);

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    if (busy) return;
    if (minimum !== null && password.length < minimum) {
      setError(
        tr(
          `Choose a master password of at least ${String(minimum)} characters.`,
          `主密码至少要 ${String(minimum)} 个字符。`,
        ),
      );
      return;
    }
    if (password !== confirm) {
      setError(tr('The two passwords do not match.', '两次输入的不一样。'));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      onInitialized(await vaultInitialize(password));
    } catch (raw) {
      setError(
        raw instanceof IpcError
          ? tr(...ipcErrorCopy(raw))
          : tr('The vault was not set up.', '保险库没建起来。'),
      );
    } finally {
      setBusy(false);
      setPassword('');
      setConfirm('');
    }
  };

  return (
    <section className="tvv-card" aria-labelledby="tvv-setup-title">
      <div className="tvv-card-head">
        <Mascot size={32} />
        <span className="tpi-eyebrow">{tr('Vault · not set up', '保险库 · 还没建')}</span>
      </div>
      <h1 id="tvv-setup-title" className="tvv-lead">
        {tr('Set up the vault', '设置保险库')}
      </h1>
      <p className="tvv-text">
        {tr(
          'What goes in here is encrypted on this Mac before it is saved, and never enters search.',
          '放进来的东西先在这台 Mac 上加密再保存,也从不进搜索。',
        )}
      </p>
      <form onSubmit={(event) => void submit(event)}>
        <label className="tvv-field">
          <span className="tvv-field-label">{tr('Master password', '主密码')}</span>
          <span className="tvv-line is-secret">
            <input
              type="password"
              autoComplete="new-password"
              aria-label={tr('Master password', '主密码')}
              value={password}
              disabled={busy}
              onChange={(event) => setPassword(event.target.value)}
            />
          </span>
        </label>
        <label className="tvv-field">
          <span className="tvv-field-label">{tr('Once more', '再输一遍')}</span>
          <span className="tvv-line is-secret">
            <input
              type="password"
              autoComplete="new-password"
              aria-label={tr('Confirm master password', '确认主密码')}
              value={confirm}
              disabled={busy}
              onChange={(event) => setConfirm(event.target.value)}
            />
          </span>
        </label>
        {/* Enter submits from either line. */}
        <button type="submit" hidden />
      </form>
      <p className="tvv-hint" role={error === null ? undefined : 'alert'}>
        {error ??
          tr(
            'There is no recovery if you forget it — Typvia never sees your master password.',
            '忘了就找不回来 —— Typvia 永远看不到你的主密码。',
          )}
      </p>
      <div className="tvv-card-foot">
        <TextAction primary onClick={() => void submit()}>
          {tr('Create the vault', '建好保险库')}
        </TextAction>
      </div>
    </section>
  );
}

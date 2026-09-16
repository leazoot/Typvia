// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { vaultUnlockBiometric, vaultUnlockPassword, type Snippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { KeyCap, TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';

interface PanelVerifyProps {
  snippet: Snippet;
  action: 'insert' | 'copy';
  destination: string | null;
  /** Called once an unlock succeeds — the parent then performs the action. */
  onUnlocked: () => void;
  onCancel: () => void;
}

/**
 * A vault snippet asked for while the vault is locked: the unlock takes the
 * list's place inside the panel (Touch ID or the master password). A failed
 * unlock never says which part was wrong and leads with what is still safe.
 * esc goes back to the list.
 */
export function PanelVerify({
  snippet,
  action,
  destination,
  onUnlocked,
  onCancel,
}: PanelVerifyProps) {
  const tr = useTr();
  const [password, setPassword] = useState('');
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);
  const inserting = action === 'insert';

  const unlock = async (run: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setFailed(false);
    try {
      await run();
      onUnlocked();
    } catch {
      // Undifferentiated by design: never reveal which factor was wrong.
      setFailed(true);
      setBusy(false);
    }
  };

  const submitPassword = () => {
    if (password.trim() === '') return;
    void unlock(() => vaultUnlockPassword(password));
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      submitPassword();
    }
  };

  return (
    <div className="tvq-fill" onKeyDown={onKeyDown}>
      <div className="tvq-head">
        <Mascot state="locked" size={22} />
        <span className="tvq-head-title">{snippet.title}</span>
        <span className="tvq-spacer" />
        {destination !== null && <span className="tvq-foot-note">→ {destination}</span>}
      </div>

      <div className="tvq-fields">
        <p className="tvq-lead">
          {inserting
            ? tr('Unlock the vault to insert this secret.', '解锁保险库后插入这条密文。')
            : tr('Unlock the vault to copy this secret.', '解锁保险库后复制这条密文。')}
        </p>
        <label className="tvq-field">
          <span className="tvq-field-label">{tr('Master password', '主密码')}</span>
          <input
            type="password"
            className="tvq-field-line"
            aria-label={tr('Master password', '主密码')}
            value={password}
            autoFocus
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        {/* Generic in both languages: never says which factor was wrong. */}
        {failed && (
          <p className="tvq-notice" role="alert">
            {tr(
              'Your vault is still safe — that didn’t unlock it. Try again.',
              '你的保险库仍然安全——这次没能解锁,再试一次。',
            )}
          </p>
        )}
        <div className="tvq-actions">
          <TextAction primary disabled={busy || password.trim() === ''} onClick={submitPassword}>
            {tr('Unlock', '解锁')}
          </TextAction>
          <TextAction disabled={busy} onClick={() => void unlock(() => vaultUnlockBiometric())}>
            {tr('Use Touch ID', '使用 Touch ID')}
          </TextAction>
        </div>
      </div>

      <footer className="tvq-foot is-bare">
        <KeyCap>⏎</KeyCap>
        <span className="tvq-foot-word">{tr('unlock', '解锁')}</span>
        <span className="tvq-spacer" />
        <span className="tvq-foot-note">{tr('esc to go back', 'esc 返回')}</span>
      </footer>
    </div>
  );
}

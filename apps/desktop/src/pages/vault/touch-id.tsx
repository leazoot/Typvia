// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { vaultEnableBiometric } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { GroupTitle } from '../../paper/kit';

/** Remembers that the Touch ID offer was already made once (a UI preference). */
const TOUCH_ID_ASKED_KEY = 'tv.ui.vaultTouchIdAsked';

function useEnrol() {
  const [state, setState] = useState<'idle' | 'done' | 'error'>('idle');
  const enrol = (then?: () => void) =>
    vaultEnableBiometric()
      .then(() => {
        setState('done');
        then?.();
      })
      .catch(() => setState('error'));
  return { state, enrol };
}

/** Offered once, right after an unlock; after that it lives in the lines below. */
export function TouchIdOffer() {
  const tr = useTr();
  const [asked, setAsked] = useState(() => localStorage.getItem(TOUCH_ID_ASKED_KEY) === 'true');
  const { state, enrol } = useEnrol();
  const remember = () => {
    localStorage.setItem(TOUCH_ID_ASKED_KEY, 'true');
    setAsked(true);
  };

  if (asked || state === 'done') return null;
  return (
    <div className="tvv-offer">
      <span>{tr('Use Touch ID next time?', '下次用 Touch ID 解锁?')}</span>
      <button type="button" className="tpi-note-yes" onClick={() => void enrol(remember)}>
        {tr('Enable', '启用')}
      </button>
      <button type="button" className="tpi-note-no" onClick={remember}>
        {tr('Not now', '不用了')}
      </button>
      {state === 'error' && (
        <span className="tvv-hint">
          {tr(
            'Touch ID is unavailable here — the master password still works.',
            '这里用不了 Touch ID —— 主密码照样能用。',
          )}
        </span>
      )}
    </div>
  );
}

/** What the vault promises, said plainly, with Touch ID enrolment for good. */
export function VaultPolicy({ minutes }: { minutes: number }) {
  const tr = useTr();
  const { state, enrol } = useEnrol();
  const m = String(minutes);
  return (
    <section
      className="tvv-policy"
      aria-label={tr('How the vault keeps them', '保险库怎么守着它们')}
    >
      <GroupTitle>{tr('How the vault keeps them', '保险库怎么守着它们')}</GroupTitle>
      <p>
        {tr(`It locks itself after ${m} minutes without activity.`, `闲置 ${m} 分钟后自动锁上。`)}
      </p>
      <p>
        {tr(
          'Contents are encrypted on this Mac and never enter the search index.',
          '内容在这台 Mac 上加密,从不进搜索索引。',
        )}
      </p>
      <p>
        {tr(
          'AI actions never read vault contents; the sync server only ever holds ciphertext.',
          'AI 动作读不到保险库;同步服务器上只有密文。',
        )}
      </p>
      <p>
        {tr(
          'A copied secret clears from the clipboard by itself.',
          '复制出去的密文会自己从剪贴板清掉。',
        )}
      </p>
      <p>
        {state === 'done'
          ? tr('Touch ID is on for this vault.', '这个保险库已经能用 Touch ID 解锁。')
          : tr('Touch ID can unlock this vault.', 'Touch ID 可以解锁这个保险库。')}
        {state !== 'done' && (
          <button type="button" className="tvv-word tvv-policy-action" onClick={() => void enrol()}>
            {tr('Enable Touch ID', '开启 Touch ID')}
          </button>
        )}
      </p>
      {state === 'error' && (
        <p className="tvv-hint">
          {tr(
            'Touch ID is unavailable here — the master password still works.',
            '这里用不了 Touch ID —— 主密码照样能用。',
          )}
        </p>
      )}
    </section>
  );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { espansoStatus } from '@typvia/shared';
import type { EspansoStatus } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useEnterAdvances } from './onboarding-page';

interface DoneStepProps {
  /** Persists the marker, then optionally routes (e.g. '/settings'). */
  onFinish: (destination?: string) => void;
}

/**
 * Step 5 — that's the whole product. Optional capabilities are flat rows with
 * true status dots and a real "Later"; nothing is demanded. The design's
 * phone-pairing row is left out until the mobile apps ship.
 */
export function DoneStep({ onFinish }: DoneStepProps) {
  const tr = useTr();
  const [espanso, setEspanso] = useState<EspansoStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    espansoStatus()
      .then((status) => {
        if (!cancelled) setEspanso(status);
      })
      .catch(() => {
        // Row falls back to the not-installed offer; the status is re-checked
        // on the Settings page it links to.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEnterAdvances(() => onFinish());

  const espansoOn = espanso !== null && espanso.enabled && espanso.state === 'running';

  return (
    <>
      <section className="tv-onb-body">
        <div className="tv-onb-done-head">
          <span aria-hidden="true" className="tv-onb-done-bar" />
          <h1 className="tv-onb-title">{tr("That's the whole product.", '这就是产品的全部。')}</h1>
        </div>
        <ul className="tv-onb-rows" aria-label={tr('Optional capabilities', '可选能力')}>
          <li className="tv-onb-row">
            <span aria-hidden="true" className="tv-onb-dot is-ok" />
            <span className="tv-onb-row-text">
              <span className="tv-onb-row-title">
                {tr('Panel works everywhere', '面板在任何地方都可用')}
              </span>
              <span className="tv-onb-row-sub">
                {tr(
                  'You just used it. That is the whole loop.',
                  '你刚刚用过了。整个流程就是这样。',
                )}
              </span>
            </span>
            <kbd className="tv-onb-row-chip">⌘⇧V</kbd>
          </li>
          <li className="tv-onb-row">
            <span
              aria-hidden="true"
              className={espansoOn ? 'tv-onb-dot is-ok' : 'tv-onb-dot is-pending'}
            />
            <span className="tv-onb-row-text">
              <span className="tv-onb-row-title">
                {tr('Type a trigger instead of opening anything', '输入触发词，无需打开任何东西')}
              </span>
              <span className="tv-onb-row-sub">
                {espansoOn
                  ? tr(
                      'The expansion engine is running — abbreviations expand as you type.',
                      '展开引擎正在运行——缩写会在你输入时自动展开。',
                    )
                  : tr(
                      'Built in — one switch in Settings turns it on.',
                      '引擎已内置——在设置中开启即可。',
                    )}
              </span>
            </span>
            <button type="button" className="tv-onb-ghost" onClick={() => onFinish('/settings')}>
              {espansoOn ? tr('Manage', '管理') : tr('Set up now', '立即设置')}
            </button>
          </li>
          <li className="tv-onb-row">
            <span aria-hidden="true" className="tv-onb-dot is-secure" />
            <span className="tv-onb-row-text">
              <span className="tv-onb-row-title">
                {tr('A place for keys and passwords', '存放密钥与密码的地方')}
              </span>
              <span className="tv-onb-row-sub">
                {tr(
                  'Masked, verified on each use, never sent to AI. Set it up when you have one to store.',
                  '始终打码，每次使用都需验证，永远不会发送给 AI。等你有需要存放的内容时再设置。',
                )}
              </span>
            </span>
            <span className="tv-onb-row-later">{tr('Later', '稍后')}</span>
          </li>
        </ul>
      </section>
      <footer className="tv-onb-foot">
        <button type="button" className="tv-onb-primary" onClick={() => onFinish()}>
          {tr('Start using Typvia', '开始使用 Typvia')}
          <kbd>↵</kbd>
        </button>
        <span>
          {tr('Everything above lives in Settings too.', '以上内容也都可以在设置中找到。')}
        </span>
      </footer>
    </>
  );
}

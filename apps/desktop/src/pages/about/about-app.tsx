// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { getVersion } from '@tauri-apps/api/app';
import { libraryCounts } from '@typvia/shared';
import { counted, useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import CHANGELOG from '../../../../../CHANGELOG.md?raw';
import NOTICES from '../../../../../THIRD_PARTY_NOTICES.md?raw';
import { Mascot } from '../../paper/mascot';
import './about.css';

/** Window label of the About window (must match tauri.conf.json). */
export const ABOUT_WINDOW_LABEL = 'about';

type Shown = 'none' | 'changes' | 'notices';

/**
 * The About window: the mark, the version, how many snippets this machine
 * holds, and the changelog and licences read in place. It cannot be
 * minimised or zoomed; closing it only puts it away.
 */
export function AboutApp() {
  const tr = useTr();
  const [version, setVersion] = useState<string | null>(null);
  const [total, setTotal] = useState<number | null>(null);
  const [shown, setShown] = useState<Shown>('none');

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
    // The window lives hidden between visits, so the count is read on every return.
    const count = () => {
      libraryCounts()
        .then((counts) => setTotal(counts.total))
        .catch(() => setTotal(null));
    };
    count();
    window.addEventListener('focus', count);
    return () => window.removeEventListener('focus', count);
  }, []);

  const toggle = (next: Shown) => setShown((current) => (current === next ? 'none' : next));

  return (
    <main className="tpi tvab">
      <div className="tvab-bar" data-tauri-drag-region="" />
      <div className="tvab-body">
        <div className="tvab-mascot">
          <Mascot state="happy" size={72} />
        </div>
        <div className="tvab-mark">TYPVIA</div>
        {version !== null && <div className="tvab-version">{version}</div>}
        {total !== null && (
          <p className="tvab-facts">
            {counted(
              tr,
              total,
              'snippet, all on this Mac',
              'snippets, all on this Mac',
              `${String(total)} 条片段,都在这台 Mac 上`,
            )}
          </p>
        )}
        <div className="tvab-rule" />
        <div className="tvab-links">
          <button
            type="button"
            className="tvab-link"
            aria-expanded={shown === 'changes'}
            onClick={() => toggle('changes')}
          >
            {tr('Changelog', '更新日志')}
          </button>
          <button
            type="button"
            className="tvab-link"
            aria-expanded={shown === 'notices'}
            onClick={() => toggle('notices')}
          >
            {tr('Licences & thanks', '许可与致谢')}
          </button>
        </div>
        {shown !== 'none' && (
          <pre
            className="tvab-text"
            tabIndex={0}
            aria-label={
              shown === 'changes'
                ? tr('Changelog', '更新日志')
                : tr('Licences & thanks', '许可与致谢')
            }
          >
            {shown === 'changes' ? CHANGELOG : NOTICES}
          </pre>
        )}
        <p className="tvab-foot">
          {tr(
            '© 2026 Typvia · snippets live only on your own machines and account',
            '© 2026 Typvia · 片段只存在你自己的机器和账户里',
          )}
        </p>
      </div>
    </main>
  );
}

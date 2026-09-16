// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { useNavigate } from 'react-router';
import { GroupTitle, KeyCap } from '../../paper/kit';

/** The keys used most; the full table is one click away. */
export function ShortcutsGroup() {
  const tr = useTr();
  const navigate = useNavigate();
  const rows = [
    { label: tr('Summon Quick Bar', '呼出 Quick Bar'), keys: '⌘⇧V' },
    { label: tr('New snippet', '新建片段'), keys: '⌘⇧N' },
    { label: tr('Command palette', '命令面板'), keys: '⌘K' },
    { label: tr('Lock the vault', '锁上保险库'), keys: '⌘L' },
    { label: tr('Open the trash', '打开回收站'), keys: '⌘⇧⌫' },
  ];

  return (
    <section className="tvs-group">
      <GroupTitle
        trailing={
          <button
            type="button"
            className="tvs-group-action"
            onClick={() => void navigate('/shortcuts')}
          >
            {tr('All shortcuts', '全部快捷键')}
          </button>
        }
      >
        {tr('Shortcuts', '快捷键')}
      </GroupTitle>
      <div className="tvs-keys">
        {rows.map((row) => (
          <div key={row.keys} className="tvs-key-row">
            <span className="tvs-key-name">{row.label}</span>
            <KeyCap>{row.keys}</KeyCap>
          </div>
        ))}
      </div>
    </section>
  );
}

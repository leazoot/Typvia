// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { summonShortcut } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { KeyCap } from '../../paper/kit';
import './shortcuts.css';

interface KeyRow {
  action: string;
  mac: string;
  windows: string;
  note?: string;
}

/**
 * Only keys that work today; macOS as symbols, Windows as words. Windows keeps
 * Win Shift V for its own clipboard history, so the summon key there is
 * whatever the host managed to register — it says which.
 */
function keyRows(tr: Tr, summon: string | null): KeyRow[] {
  return [
    {
      action: tr('Summon Quick Bar', '呼出 Quick Bar'),
      mac: '⌘⇧V',
      windows: 'Ctrl Alt V',
      note:
        summon === null
          ? tr('this machine granted none', '这台机器没把呼出键给出来')
          : tr(`in any app · ${summon} here`, `在任何应用里 · 本机是 ${summon}`),
    },
    { action: tr('Insert the picked one', '插入选中项'), mac: '⏎', windows: 'Enter' },
    { action: tr('Copy, do not insert', '只复制不插入'), mac: '⌥⏎', windows: 'Alt Enter' },
    { action: tr('Fill variables / next one', '填变量 / 下一格'), mac: '⇥', windows: 'Tab' },
    {
      action: tr('Insert a row of Quick Bar', '插入 Quick Bar 的第几行'),
      mac: '⌘1 – ⌘9',
      windows: 'Ctrl 1 – Ctrl 9',
    },
    { action: tr('New snippet', '新建片段'), mac: '⌘⇧N', windows: 'Ctrl Shift N' },
    { action: tr('Search snippets', '搜索片段'), mac: '⌘F', windows: 'Ctrl F' },
    { action: tr('Command palette', '命令面板'), mac: '⌘K', windows: 'Ctrl K' },
    { action: tr('New collection', '新建集合'), mac: '⌘⇧K', windows: 'Ctrl Shift K' },
    { action: tr('Lock the vault', '锁上保险库'), mac: '⌘L', windows: 'Ctrl L' },
    {
      action: tr('Delete the picked snippet', '删除选中的片段'),
      mac: '⌘⌫',
      windows: 'Ctrl Backspace',
    },
    {
      action: tr('Undo the last delete', '撤回刚删掉的'),
      mac: '⌘Z',
      windows: 'Ctrl Z',
      note: tr('outside a text field', '不在输入框里时'),
    },
    { action: tr('Open the trash', '打开回收站'), mac: '⌘⇧⌫', windows: 'Ctrl Shift Backspace' },
    { action: tr('Settings', '设置'), mac: '⌘,', windows: 'Ctrl ,' },
    {
      action: tr('Library · Vault · AI actions', '资料库 · 保险库 · AI 动作'),
      mac: '⌘1 · ⌘2 · ⌘3',
      windows: 'Ctrl 1 · Ctrl 2 · Ctrl 3',
    },
  ];
}

export function ShortcutsPage() {
  const tr = useTr();
  // Only the host knows which keys the system handed over.
  const [summon, setSummon] = useState<string | null>(null);
  useEffect(() => {
    summonShortcut()
      .then(setSummon)
      .catch(() => setSummon(null));
  }, []);

  return (
    <div className="tpi tvk">
      <div className="tvk-sheet">
        <h1 className="tvk-title">{tr('Every shortcut', '快捷键总览')}</h1>
        <p className="tvk-lede">
          {tr(
            'macOS keys as symbols, Windows keys as words — one table, two columns.',
            'macOS 用符号键帽,Windows 用文字键帽 —— 同一张表两列写完。',
          )}
        </p>
        <div className="tvk-table" role="table" aria-label={tr('Shortcuts', '快捷键')}>
          <div className="tvk-row is-head" role="row">
            <div role="columnheader">{tr('ACTION', '动作')}</div>
            <div role="columnheader">MACOS</div>
            <div role="columnheader">WINDOWS</div>
          </div>
          {keyRows(tr, summon).map((row) => (
            <div key={row.mac} className="tvk-row" role="row">
              <div role="rowheader">{row.action}</div>
              <div role="cell">
                <KeyCap>{row.mac}</KeyCap>
              </div>
              <div role="cell">
                <KeyCap>{row.windows}</KeyCap>
                {row.note !== undefined && <span className="tvk-note">{row.note}</span>}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

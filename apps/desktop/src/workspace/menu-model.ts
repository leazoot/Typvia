// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Tr } from '@typvia/ui';
import type { AppCommand } from './commands';

/** What a menu row asks for: a place to open, a window-level act, or a page command. */
export type MenuAction =
  | `open:${string}`
  | `snippet.move:${string}`
  | 'snippet.new'
  | 'snippet.from-clipboard'
  | 'vault.lock'
  | 'trash.undo'
  | 'sync.now'
  | 'about.open'
  | Exclude<AppCommand, 'snippet.move'>;

/** Everything the rows depend on; the shell re-sends it whenever any of it changes. */
export interface MenuStatus {
  route: string;
  vaultUnlocked: boolean;
  /** The vault menu's closing line: how many, and when it locks. */
  vaultLine: string;
  trashCount: number;
  /** The trash menu's closing line: how many, and the oldest one's days left. */
  trashLine: string;
  canUndo: boolean;
  syncOn: boolean;
  collections: ReadonlyArray<{ id: string; name: string }>;
}

export type MenuPart =
  | {
      kind: 'row';
      label: string;
      action: MenuAction;
      /** In Tauri's accelerator syntax, e.g. "CmdOrCtrl+Shift+N". */
      accelerator?: string;
      /** Absent means the row can always run. */
      enabled?: (status: MenuStatus) => boolean;
    }
  | { kind: 'separator' }
  /** "Move to collection": one row per collection, rebuilt as they change. */
  | { kind: 'collections'; label: string; enabled: (status: MenuStatus) => boolean }
  /** A closing line that reports a number; never clickable. */
  | { kind: 'line'; text: (status: MenuStatus) => string };

export interface MenuSection {
  title: string;
  parts: readonly MenuPart[];
}

const SEPARATOR: MenuPart = { kind: 'separator' };

function row(
  label: string,
  action: MenuAction,
  accelerator?: string,
  enabled?: (status: MenuStatus) => boolean,
): MenuPart {
  return {
    kind: 'row',
    label,
    action,
    ...(accelerator === undefined ? {} : { accelerator }),
    ...(enabled === undefined ? {} : { enabled }),
  };
}

const onLibrary = (status: MenuStatus) => status.route === '/';
const onTrashWithRows = (status: MenuStatus) => status.route === '/trash' && status.trashCount > 0;

/**
 * The four menus — Snippets, Vault, Trash, Settings — as data for the macOS
 * menu bar.
 *
 * Keys the page itself needs (⏎, ⌘⌫, ⌘Z, ⌘⇧V) carry no accelerator: a menu
 * key equivalent would take them from every text field and from the summon
 * shortcut. Rows that act on "the picked one" run only on the page that has a
 * pick.
 */
export function menuSections(tr: Tr): MenuSection[] {
  return [
    {
      title: tr('Snippets', '片段'),
      parts: [
        row(tr('New snippet', '新建片段'), 'snippet.new', 'CmdOrCtrl+Shift+N'),
        row(tr('New from the clipboard', '用剪贴板里的内容新建'), 'snippet.from-clipboard'),
        row(tr('Back to the library', '回到资料库'), 'open:/', 'CmdOrCtrl+1'),
        SEPARATOR,
        row(tr('Edit', '编辑'), 'snippet.edit', undefined, onLibrary),
        row(
          tr('Copy as plain text', '复制为纯文本'),
          'snippet.copy-plain',
          'Alt+CmdOrCtrl+C',
          onLibrary,
        ),
        { kind: 'collections', label: tr('Move to collection', '移到集合'), enabled: onLibrary },
        row(
          tr('Add to the vault', '加到保险库'),
          'snippet.to-vault',
          'CmdOrCtrl+Shift+L',
          (status) => onLibrary(status) && status.vaultUnlocked,
        ),
        SEPARATOR,
        row(tr('Export all snippets…', '导出全部片段…'), 'open:/settings'),
        row(tr('Import snippets…', '导入片段…'), 'open:/settings'),
        SEPARATOR,
        row(tr('Delete snippet', '删除片段'), 'snippet.delete', undefined, onLibrary),
      ],
    },
    {
      title: tr('Vault', '保险库'),
      parts: [
        row(
          tr('Unlock the vault', '解锁保险库'),
          'open:/vault',
          'CmdOrCtrl+Shift+U',
          (status) => !status.vaultUnlocked,
        ),
        row(tr('Lock', '锁上'), 'vault.lock', 'CmdOrCtrl+L', (status) => status.vaultUnlocked),
        SEPARATOR,
        row(
          tr('Take the picked snippet in', '把选中的片段收进来'),
          'snippet.to-vault',
          undefined,
          (status) => onLibrary(status) && status.vaultUnlocked,
        ),
        row(
          tr('Peek at the picked one', '看一眼选中的这条'),
          'vault.peek',
          'Alt+CmdOrCtrl+L',
          (status) => status.route === '/vault' && status.vaultUnlocked,
        ),
        SEPARATOR,
        { kind: 'line', text: (status) => status.vaultLine },
      ],
    },
    {
      title: tr('Trash', '回收站'),
      parts: [
        row(tr('Open the trash', '打开回收站'), 'open:/trash', 'CmdOrCtrl+Shift+Backspace'),
        row(
          tr('Undo the last delete', '撤回刚删掉的那条'),
          'trash.undo',
          undefined,
          (status) => status.canUndo,
        ),
        SEPARATOR,
        row(
          tr('Restore the picked one', '找回选中的片段'),
          'trash.restore-picked',
          undefined,
          onTrashWithRows,
        ),
        row(
          tr('Delete the picked one for good', '彻底删掉选中的'),
          'trash.purge-picked',
          undefined,
          onTrashWithRows,
        ),
        SEPARATOR,
        row(
          tr('Empty the trash', '清空回收站'),
          'trash.empty',
          undefined,
          (status) => status.trashCount > 0,
        ),
        { kind: 'line', text: (status) => status.trashLine },
      ],
    },
    {
      title: tr('Settings', '设置'),
      parts: [
        row(tr('All settings…', '全部设置…'), 'open:/settings'),
        SEPARATOR,
        row(tr('Sync once now', '现在同步一次'), 'sync.now', undefined, (status) => status.syncOn),
        row(tr('Sync & devices', '同步与设备'), 'open:/sync'),
        SEPARATOR,
        row(tr('AI actions', 'AI 动作'), 'open:/ai', 'CmdOrCtrl+3'),
        row(tr('Template builder', '模板编辑器'), 'open:/templates'),
        row(tr('All shortcuts', '快捷键总览'), 'open:/shortcuts'),
        SEPARATOR,
        row(tr('About Typvia', '关于 Typvia'), 'about.open'),
      ],
    },
  ];
}

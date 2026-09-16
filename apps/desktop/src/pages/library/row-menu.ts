// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Folder, Snippet, VaultStatus } from '@typvia/shared';
import type { Tr } from '@typvia/ui';
import type { MenuEntry } from '../../paper/menu';

export interface RowActions {
  insert: () => void;
  /** Opens the template's variables in the detail pane. */
  fill: () => void;
  copy: () => void;
  edit: () => void;
  editTrigger: () => void;
  /** '' moves it to Unsorted. */
  moveTo: (folderId: string) => void;
  toVault: () => void;
  remove: () => void;
}

/**
 * The right-click menu on a snippet row. Rows that cannot run say why on the
 * row, instead of disappearing and leaving the reader to wonder.
 */
export function rowMenu({
  snippet,
  hasVariables,
  folders,
  vault,
  actions,
  tr,
}: {
  snippet: Snippet;
  hasVariables: boolean;
  folders: readonly Folder[];
  vault: VaultStatus | null;
  actions: RowActions;
  tr: Tr;
}): MenuEntry[] {
  const here = snippet.folderId ?? '';
  const collections: Array<{ id: string; name: string }> = [
    ...folders.map((folder) => ({ id: folder.id, name: folder.name })),
    { id: '', name: tr('Unsorted', '待整理') },
  ];
  const moveEntries: MenuEntry[] = collections.map((collection) =>
    collection.id === here
      ? { kind: 'item', label: collection.name, reason: tr("It's here", '就在这里') }
      : { kind: 'item', label: collection.name, onSelect: () => actions.moveTo(collection.id) },
  );

  const vaultReason =
    vault === null || !vault.initialized
      ? tr('No vault yet', '还没建保险库')
      : !vault.unlocked
        ? tr('Unlock it first', '先解锁保险库')
        : undefined;

  return [
    {
      kind: 'item',
      label: tr('Insert into the front app', '插入到最前应用'),
      keys: '⏎',
      // A template's braces must never be typed out raw: it asks for its values first.
      onSelect: hasVariables ? actions.fill : actions.insert,
    },
    {
      kind: 'item',
      label: tr('Copy, do not insert', '只复制不插入'),
      keys: '⌥⏎',
      onSelect: actions.copy,
    },
    hasVariables
      ? {
          kind: 'item',
          label: tr('Fill variables, then insert', '填变量后插入'),
          onSelect: actions.fill,
        }
      : {
          kind: 'item',
          label: tr('Fill variables, then insert', '填变量后插入'),
          reason: tr('No variables', '没有变量'),
        },
    { kind: 'separator' },
    { kind: 'item', label: tr('Edit', '编辑'), onSelect: actions.edit },
    { kind: 'item', label: tr('Change the trigger', '改触发词'), onSelect: actions.editTrigger },
    { kind: 'item', label: tr('Move to collection', '移到集合'), submenu: moveEntries },
    vaultReason === undefined
      ? { kind: 'item', label: tr('Add to the vault', '加到保险库'), onSelect: actions.toVault }
      : { kind: 'item', label: tr('Add to the vault', '加到保险库'), reason: vaultReason },
    { kind: 'separator' },
    {
      kind: 'item',
      label: tr('Delete snippet', '删除片段'),
      keys: '⌘⌫',
      danger: true,
      onSelect: actions.remove,
    },
  ];
}

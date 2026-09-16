// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The four workspace rooms, in menu and ⌘-digit order. The Library is the
 * main window itself, so it lives at the root.
 */
export interface Room {
  readonly path: string;
  /** Two-character mono mark. */
  readonly mark: string;
  readonly labelEn: string;
  readonly labelZh: string;
  readonly descEn: string;
  readonly descZh: string;
  /** Digit of the ⌘-shortcut that jumps here. */
  readonly digit: number;
}

export const ROOMS: readonly Room[] = [
  {
    path: '/',
    mark: 'TX',
    labelEn: 'Library',
    labelZh: '资料库',
    descEn: 'Search and organise your snippets',
    descZh: '搜索与管理普通片段',
    digit: 1,
  },
  {
    path: '/vault',
    mark: 'SC',
    labelEn: 'Vault',
    labelZh: '保险库',
    descEn: 'Sensitive snippets, encrypted on this Mac',
    descZh: '本机加密的敏感片段',
    digit: 2,
  },
  {
    path: '/ai',
    mark: 'AI',
    labelEn: 'AI Actions',
    labelZh: 'AI 动作',
    descEn: 'Text actions you compose yourself',
    descZh: '创建文本处理动作',
    digit: 3,
  },
  {
    path: '/trash',
    mark: '↺',
    labelEn: 'Trash',
    labelZh: '回收站',
    descEn: 'Recently deleted snippets',
    descZh: '最近删除的片段',
    digit: 4,
  },
];

export function roomFor(pathname: string): Room | undefined {
  return ROOMS.find(
    (room) => pathname === room.path || (room.path !== '/' && pathname.startsWith(`${room.path}/`)),
  );
}

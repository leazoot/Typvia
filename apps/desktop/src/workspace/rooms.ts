/**
 * The four workspace rooms behind the switcher, in switcher order. Home and
 * Settings are not rooms — they are the two fixed ends of the navigation.
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
    path: '/library',
    mark: 'TX',
    labelEn: 'Library',
    labelZh: '片段库',
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
  return ROOMS.find((room) => pathname === room.path || pathname.startsWith(`${room.path}/`));
}

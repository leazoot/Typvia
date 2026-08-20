/**
 * Top-level application routes shared by the router and navigation.
 *
 * Each route carries its English and Chinese wording so the active locale can
 * be rendered on its own; paths are app-internal.
 */
export interface AppRoute {
  readonly path: string;
  readonly labelEn: string;
  readonly labelCn: string;
}

export const APP_ROUTES: readonly AppRoute[] = [
  { path: '/', labelEn: 'Home', labelCn: '首页' },
  { path: '/library', labelEn: 'Library', labelCn: '片段库' },
  { path: '/editor', labelEn: 'Snippet editor', labelCn: '片段编辑' },
  { path: '/templates', labelEn: 'Template builder', labelCn: '模板编辑器' },
  { path: '/vault', labelEn: 'Vault', labelCn: '保险库' },
  { path: '/ai', labelEn: 'AI Actions', labelCn: 'AI 动作' },
  { path: '/sync', labelEn: 'Sync & devices', labelCn: '同步与设备' },
  { path: '/trash', labelEn: 'Trash', labelCn: '回收站' },
  { path: '/settings', labelEn: 'Settings', labelCn: '设置' },
];

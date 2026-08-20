import type {
  AiActionInputSource,
  AiActionOutputMode,
  AiActionPermissionScope,
} from '@typvia/shared';
import type { Tr } from '../i18n';

/**
 * The action vocabulary both hosts print: every label
 * names a consequence, and the confirm label names the exact application so
 * "nothing applied until you confirm" reads as a promise, not a caveat.
 */

export function sourceLabel(source: AiActionInputSource, tr: Tr): string {
  const labels: Record<AiActionInputSource, string> = {
    selection: tr('Current selection', '当前选中内容'),
    clipboard: tr('Clipboard', '剪贴板'),
    snippet: tr('Snippet body', '片段正文'),
    share: tr('Shared text', '分享的文本'),
  };
  return labels[source];
}

export function modeLabel(mode: AiActionOutputMode, tr: Tr): string {
  const labels: Record<AiActionOutputMode, string> = {
    replace: tr('Replace, after confirm', '确认后替换'),
    insert: tr('Insert, after confirm', '确认后插入'),
    copy: tr('Copy, after confirm', '确认后复制'),
    new_snippet: tr('New snippet, after confirm', '确认后新建片段'),
  };
  return labels[mode];
}

export function scopeLabel(scope: AiActionPermissionScope, tr: Tr): string {
  const labels: Record<AiActionPermissionScope, string> = {
    normal_only: tr('Refuse to run', '拒绝运行'),
    mask_secrets: tr('Stripped before sending', '发送前剔除'),
  };
  return labels[scope];
}

export function confirmLabel(mode: AiActionOutputMode, tr: Tr): string {
  const labels: Record<AiActionOutputMode, string> = {
    replace: tr('Replace input', '替换输入'),
    insert: tr('Insert below input', '插入到输入之后'),
    copy: tr('Copy result', '复制结果'),
    new_snippet: tr('Save as snippet', '存为片段'),
  };
  return labels[mode];
}

export function countWords(text: string): number {
  return text.split(/\s+/).filter(Boolean).length;
}

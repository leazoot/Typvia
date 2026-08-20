import type { Locale } from '@typvia/ui';

/**
 * Row meta-line vocabulary: the snippet type
 * as a word — "COMMAND · 2 MIN" — never a chip or a coloured tag. English
 * caps come from CSS (`text-transform`), so the words here stay readable to
 * screen readers.
 */
const TYPE_WORD_EN: Record<string, string> = {
  text: 'Text',
  markdown: 'Text',
  code: 'Code',
  command: 'Command',
  prompt: 'Prompt',
  template: 'Template',
  sensitive: 'Secret',
  ai_action: 'AI',
  link: 'Link',
  temporary: 'Text',
};

const TYPE_WORD_ZH: Record<string, string> = {
  text: '文本',
  markdown: '文本',
  code: '代码',
  command: '命令',
  prompt: '提示词',
  template: '模板',
  sensitive: '密钥',
  ai_action: 'AI',
  link: '链接',
  temporary: '文本',
};

export function typeWord(snippetType: string, locale: Locale): string {
  const table = locale === 'zh' ? TYPE_WORD_ZH : TYPE_WORD_EN;
  return table[snippetType] ?? table['text']!;
}

/** Machine content previews in mono; prose previews in the sans stack. */
export function previewIsMono(snippetType: string): boolean {
  return (
    snippetType === 'code' ||
    snippetType === 'command' ||
    snippetType === 'template' ||
    snippetType === 'prompt'
  );
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * Recency word for the meta line — "2 min", "yesterday", "3 days". The
 * in-between steps follow the same MIN/YESTERDAY/DAYS voice.
 */
export function recencyWord(lastUsedAt: number, now: number, locale: Locale): string {
  const zh = locale === 'zh';
  const elapsed = Math.max(0, now - lastUsedAt);
  if (elapsed < MINUTE) return zh ? '刚刚' : 'now';
  if (elapsed < HOUR) {
    const minutes = Math.floor(elapsed / MINUTE);
    return zh ? `${minutes} 分钟前` : `${minutes} min`;
  }
  if (elapsed < DAY) {
    const hours = Math.floor(elapsed / HOUR);
    return zh ? `${hours} 小时前` : `${hours} hr`;
  }
  const days = Math.floor(elapsed / DAY);
  if (days === 1) return zh ? '昨天' : 'yesterday';
  if (days < 30) return zh ? `${days} 天前` : `${days} days`;
  const months = Math.floor(days / 30);
  return zh ? `${months} 个月前` : `${months} mo`;
}

/** Sentence-length recency for the detail screen — "2 minutes ago". */
export function agoPhrase(lastUsedAt: number, now: number, locale: Locale): string {
  if (locale === 'zh') return recencyWord(lastUsedAt, now, 'zh');
  const elapsed = Math.max(0, now - lastUsedAt);
  if (elapsed < MINUTE) return 'just now';
  if (elapsed < HOUR) {
    const minutes = Math.floor(elapsed / MINUTE);
    return `${minutes} minute${minutes === 1 ? '' : 's'} ago`;
  }
  if (elapsed < DAY) {
    const hours = Math.floor(elapsed / HOUR);
    return `${hours} hour${hours === 1 ? '' : 's'} ago`;
  }
  const days = Math.floor(elapsed / DAY);
  if (days === 1) return 'yesterday';
  if (days < 30) return `${days} days ago`;
  const months = Math.floor(days / 30);
  return `${months} month${months === 1 ? '' : 's'} ago`;
}

/** "Command · 2 min" / "命令 · 2 分钟前"; type word alone when never used. */
export function stripMeta(
  snippetType: string,
  lastUsedAt: number | null,
  now: number,
  locale: Locale,
): string {
  const word = typeWord(snippetType, locale);
  return lastUsedAt === null ? word : `${word} · ${recencyWord(lastUsedAt, now, locale)}`;
}

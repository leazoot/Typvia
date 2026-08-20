import { trFor, type Locale } from '../i18n';

/**
 * Route labels read like the design's ("2 min ago", "yesterday", "Mar 4").
 * Short spans stay relative because that is what the drawing is about — how
 * recently the line moved — and older ones become a date, which is what you
 * actually recognise once "14 days ago" stops meaning anything.
 */
export function relativeTime(at: number, now: number, locale: Locale): string {
  const tr = trFor(locale);
  const elapsed = Math.max(0, now - at);
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return tr('just now', '刚刚');
  if (minutes < 60) return tr(`${String(minutes)} min ago`, `${String(minutes)} 分钟前`);
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return tr(`${String(hours)} h ago`, `${String(hours)} 小时前`);
  const days = Math.floor(hours / 24);
  if (days === 1) return tr('yesterday', '昨天');
  if (days < 7) return tr(`${String(days)} days ago`, `${String(days)} 天前`);
  return new Date(at).toLocaleDateString(locale === 'zh' ? 'zh-CN' : 'en-US', {
    month: 'short',
    day: 'numeric',
  });
}

/** "3 months ago" scale for the recovery-code line, which is about age. */
export function ageLabel(at: number, now: number, locale: Locale): string {
  const tr = trFor(locale);
  const days = Math.floor(Math.max(0, now - at) / 86_400_000);
  if (days < 1) return tr('today', '今天');
  if (days < 30) return tr(`${String(days)} days ago`, `${String(days)} 天前`);
  const months = Math.floor(days / 30);
  if (months < 12) {
    return tr(
      `${String(months)} ${months === 1 ? 'month' : 'months'} ago`,
      `${String(months)} 个月前`,
    );
  }
  const years = Math.floor(months / 12);
  return tr(`${String(years)} years ago`, `${String(years)} 年前`);
}

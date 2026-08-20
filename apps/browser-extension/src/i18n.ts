/**
 * Single-language rendering for the extension surfaces. The app's `useTr`
 * hook cannot run here, so the same inline en/zh pair contract keys off the
 * browser UI language instead.
 */

/** True when the browser UI language is Chinese. */
export function isZh(language: string): boolean {
  return language.toLowerCase().startsWith('zh');
}

/** Returns a translator over the given browser language. */
export function makeTr(language: string): (en: string, zh: string) => string {
  const zh = isZh(language);
  return (en, zhText) => (zh ? zhText : en);
}

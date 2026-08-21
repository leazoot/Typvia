// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useState,
  type ReactNode,
} from 'react';
import { I18nProvider, type Locale } from '../i18n';
import { applyTheme, type ThemeName } from '../tokens';

/**
 * UI preferences: language and appearance. Both are pure UI preferences, so
 * they live in localStorage (never snippet data) and sync across this app's
 * windows (main ↔ panel) through the `storage` event. `system` resolves
 * against the OS at read time and keeps following it live.
 */
export type LocalePref = 'system' | Locale;
export type ThemePref = 'system' | ThemeName;

export const LOCALE_PREF_KEY = 'tv.ui.locale';
export const THEME_PREF_KEY = 'tv.ui.theme';

export function loadLocalePref(): LocalePref {
  const raw = localStorage.getItem(LOCALE_PREF_KEY);
  return raw === 'en' || raw === 'zh' ? raw : 'system';
}

export function loadThemePref(): ThemePref {
  const raw = localStorage.getItem(THEME_PREF_KEY);
  return raw === 'light' || raw === 'dark' ? raw : 'system';
}

export function resolveLocale(pref: LocalePref): Locale {
  if (pref !== 'system') return pref;
  const language = typeof navigator === 'undefined' ? '' : navigator.language;
  return language.toLowerCase().startsWith('zh') ? 'zh' : 'en';
}

function systemDarkQuery(): MediaQueryList | null {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-color-scheme: dark)')
    : null;
}

export function resolveTheme(pref: ThemePref): ThemeName {
  if (pref !== 'system') return pref;
  return systemDarkQuery()?.matches ? 'dark' : 'light';
}

/**
 * Applies the stored preferences once, before React mounts, so the first
 * paint already has the right theme and `lang`. The provider takes over from
 * there.
 */
export function applyStoredUiPrefs(root: HTMLElement): void {
  applyTheme(root, resolveTheme(loadThemePref()));
  root.lang = resolveLocale(loadLocalePref()) === 'zh' ? 'zh-Hans' : 'en';
}

interface UiPrefs {
  localePref: LocalePref;
  themePref: ThemePref;
  locale: Locale;
  theme: ThemeName;
  setLocalePref: (pref: LocalePref) => void;
  setThemePref: (pref: ThemePref) => void;
}

const noop = () => undefined;
const UiPrefsContext = createContext<UiPrefs>({
  localePref: 'system',
  themePref: 'system',
  locale: 'en',
  theme: 'light',
  setLocalePref: noop,
  setThemePref: noop,
});

export function useUiPrefs(): UiPrefs {
  return useContext(UiPrefsContext);
}

export function UiPrefsProvider({ children }: { children: ReactNode }) {
  const [localePref, setLocaleState] = useState<LocalePref>(loadLocalePref);
  const [themePref, setThemeState] = useState<ThemePref>(loadThemePref);
  // Bump to re-resolve `system` prefs when the OS appearance flips.
  const [, setSystemTick] = useState(0);

  const setLocalePref = useCallback((pref: LocalePref) => {
    localStorage.setItem(LOCALE_PREF_KEY, pref);
    setLocaleState(pref);
  }, []);

  const setThemePref = useCallback((pref: ThemePref) => {
    localStorage.setItem(THEME_PREF_KEY, pref);
    setThemeState(pref);
  }, []);

  const locale = resolveLocale(localePref);
  const theme = resolveTheme(themePref);

  useLayoutEffect(() => {
    const root = document.documentElement;
    applyTheme(root, theme);
    root.lang = locale === 'zh' ? 'zh-Hans' : 'en';
  }, [theme, locale]);

  useEffect(() => {
    if (themePref !== 'system') return;
    const query = systemDarkQuery();
    if (query === null) return;
    const onChange = () => setSystemTick((tick) => tick + 1);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  }, [themePref]);

  // Another window of this app (main ↔ panel) changed a preference.
  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === LOCALE_PREF_KEY) setLocaleState(loadLocalePref());
      if (event.key === THEME_PREF_KEY) setThemeState(loadThemePref());
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, []);

  return (
    <UiPrefsContext.Provider
      value={{ localePref, themePref, locale, theme, setLocalePref, setThemePref }}
    >
      <I18nProvider locale={locale}>{children}</I18nProvider>
    </UiPrefsContext.Provider>
  );
}

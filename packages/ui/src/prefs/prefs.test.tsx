// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useTr } from '../i18n';
import {
  LOCALE_PREF_KEY,
  loadLocalePref,
  loadThemePref,
  resolveLocale,
  resolveTheme,
  THEME_PREF_KEY,
  UiPrefsProvider,
  useUiPrefs,
} from './index';

function Probe() {
  const { locale, theme, setLocalePref, setThemePref } = useUiPrefs();
  const tr = useTr();
  return (
    <div>
      <output aria-label="locale">{locale}</output>
      <output aria-label="theme">{theme}</output>
      <output aria-label="greeting">{tr('Hello', '你好')}</output>
      <button type="button" onClick={() => setLocalePref('zh')}>
        to zh
      </button>
      <button type="button" onClick={() => setThemePref('dark')}>
        to dark
      </button>
    </div>
  );
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  cleanup();
  localStorage.clear();
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.removeAttribute('lang');
});

describe('stored preference parsing', () => {
  it('falls_back_to_system_on_missing_or_garbage_values', () => {
    expect(loadLocalePref()).toBe('system');
    expect(loadThemePref()).toBe('system');
    localStorage.setItem(LOCALE_PREF_KEY, 'klingon');
    localStorage.setItem(THEME_PREF_KEY, 'sepia');
    expect(loadLocalePref()).toBe('system');
    expect(loadThemePref()).toBe('system');
  });

  it('resolves_explicit_prefs_verbatim', () => {
    expect(resolveLocale('zh')).toBe('zh');
    expect(resolveLocale('en')).toBe('en');
    expect(resolveTheme('dark')).toBe('dark');
    expect(resolveTheme('light')).toBe('light');
  });

  it('system_theme_resolves_light_when_matchMedia_is_unavailable', () => {
    // jsdom has no matchMedia; the resolver must not crash and must pick light.
    expect(resolveTheme('system')).toBe('light');
  });
});

describe('UiPrefsProvider', () => {
  it('switching_language_re_renders_strings_persists_and_stamps_lang', () => {
    render(
      <UiPrefsProvider>
        <Probe />
      </UiPrefsProvider>,
    );
    expect(screen.getByLabelText('greeting').textContent).toBe('Hello');

    fireEvent.click(screen.getByRole('button', { name: 'to zh' }));
    expect(screen.getByLabelText('greeting').textContent).toBe('你好');
    expect(localStorage.getItem(LOCALE_PREF_KEY)).toBe('zh');
    expect(document.documentElement.lang).toBe('zh-Hans');
  });

  it('switching_theme_applies_data_theme_and_persists', () => {
    render(
      <UiPrefsProvider>
        <Probe />
      </UiPrefsProvider>,
    );
    expect(screen.getByLabelText('theme').textContent).toBe('light');

    fireEvent.click(screen.getByRole('button', { name: 'to dark' }));
    expect(screen.getByLabelText('theme').textContent).toBe('dark');
    expect(document.documentElement.dataset['theme']).toBe('dark');
    expect(localStorage.getItem(THEME_PREF_KEY)).toBe('dark');
  });

  it('stored_zh_pref_renders_chinese_from_first_paint', () => {
    localStorage.setItem(LOCALE_PREF_KEY, 'zh');
    render(
      <UiPrefsProvider>
        <Probe />
      </UiPrefsProvider>,
    );
    expect(screen.getByLabelText('greeting').textContent).toBe('你好');
  });
});

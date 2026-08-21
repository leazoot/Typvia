// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Shared React component library built on the Typvia design tokens. Package entry point.
export { Caret } from './components/caret';
export { FilterChipRow } from './components/filter-chips';
export type { FilterChip } from './components/filter-chips';
export { Page } from './components/page';
export { QrCode } from './components/qr-code';
export { QR_MAX_BYTES, qrMatrix } from './components/qr-matrix';
export { SearchField } from './components/search-field';
export { SearchLine } from './components/search-line';
export type { SearchLineScale } from './components/search-line';
export { HomeNavIcon, LibraryNavIcon, SettingsNavIcon, VaultNavIcon } from './components/nav-icons';
export { SnippetStrip, SnippetStripSkeleton } from './components/snippet-strip';
export { TabBar } from './components/tab-bar';
export type { TabBarItem } from './components/tab-bar';
export { useVirtualRows } from './components/use-virtual-rows';
export { StatusDot } from './components/status-dot';
export type { StatusKind } from './components/status-dot';
export { Toast } from './components/toast';
export { markForType, TypeMark } from './components/type-mark';
export { PlaceholderPage } from './placeholder-page';
export { I18nProvider, trFor, useLocale, useTr } from './i18n';
export type { Locale, Tr } from './i18n';
export {
  applyStoredUiPrefs,
  loadLocalePref,
  loadThemePref,
  LOCALE_PREF_KEY,
  resolveLocale,
  resolveTheme,
  THEME_PREF_KEY,
  UiPrefsProvider,
  useUiPrefs,
} from './prefs';
export type { LocalePref, ThemePref } from './prefs';
export {
  applyTheme,
  BREAKPOINTS,
  CN_LEADING_DELTA,
  cnLeading,
  cssVariables,
  designTokens,
  followSystemTheme,
  TYPE_MARKS,
} from './tokens';
export type { ThemeName } from './tokens';

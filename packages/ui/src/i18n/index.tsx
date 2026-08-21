// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { createContext, useContext, type ReactNode } from 'react';

/**
 * UI language. The product renders exactly one language at a time (DEC: the
 * bilingual "EN label + CN subtitle" pattern is retired). Every user-facing
 * string is written inline at its call site as an (en, zh) pair and resolved
 * through `useTr`, so translations live next to the code they belong to.
 */
export type Locale = 'en' | 'zh';

const LocaleContext = createContext<Locale>('en');

export function I18nProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  return <LocaleContext.Provider value={locale}>{children}</LocaleContext.Provider>;
}

export function useLocale(): Locale {
  return useContext(LocaleContext);
}

/** String picker for components: `tr('Save', '保存')` renders one language. */
export type Tr = (en: string, zh: string) => string;

export function useTr(): Tr {
  return trFor(useContext(LocaleContext));
}

/** Non-hook variant for helpers that receive the locale as a value. */
export function trFor(locale: Locale): Tr {
  return locale === 'zh' ? (_en: string, zh: string) => zh : (en: string) => en;
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { createContext, useContext, type ReactNode } from 'react';

/**
 * UI language. The product renders exactly one language at a time — never an
 * English label with a Chinese subtitle beside it. Every user-facing
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

/**
 * A count and the noun it counts, with English's plural decided in one place.
 *
 * English has a plural and Chinese does not, so the Chinese half is passed
 * whole and only the English half branches. Written out at each call site the
 * branch gets forgotten — "1 results", "1 weeks", "1 years ago" — which is the
 * kind of mistake a reader reads as carelessness about everything else.
 *
 * Sentences whose verb also agrees branch at their call site instead: this
 * decides a noun, not a grammar.
 */
export function counted(tr: Tr, count: number, one: string, many: string, zh: string): string {
  return tr(`${String(count)} ${count === 1 ? one : many}`, zh);
}

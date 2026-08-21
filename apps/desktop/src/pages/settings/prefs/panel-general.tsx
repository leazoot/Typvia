// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/** General — language and appearance as segmented controls. */
import { useTr, useUiPrefs, type LocalePref, type ThemePref } from '@typvia/ui';
import { PrefSection } from './pref-section';
import { Seg, SetLabel, SetRow } from './pref-kit';

export function GeneralPref() {
  const tr = useTr();
  const { localePref, themePref, setLocalePref, setThemePref } = useUiPrefs();

  const languages: ReadonlyArray<{ id: LocalePref; label: string }> = [
    { id: 'system', label: tr('System', '跟随系统') },
    { id: 'en', label: 'English' },
    { id: 'zh', label: '中文' },
  ];
  const appearances: ReadonlyArray<{ id: ThemePref; label: string }> = [
    { id: 'system', label: tr('System', '跟随系统') },
    { id: 'light', label: tr('Light', '浅色') },
    { id: 'dark', label: tr('Dark', '深色') },
  ];

  const summary = [
    languages.find((option) => option.id === localePref)?.label,
    appearances.find((option) => option.id === themePref)?.label,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <PrefSection id="general" glyph="general" name={tr('General', '通用')} sub={summary}>
      <SetLabel>{tr('Language', '语言')}</SetLabel>
      <SetRow>
        <Seg
          label={tr('Language', '语言')}
          options={languages}
          value={localePref}
          onChange={setLocalePref}
        />
      </SetRow>
      <SetLabel>{tr('Appearance', '外观')}</SetLabel>
      <SetRow>
        <Seg
          label={tr('Appearance', '外观')}
          options={appearances}
          value={themePref}
          onChange={setThemePref}
        />
      </SetRow>
    </PrefSection>
  );
}

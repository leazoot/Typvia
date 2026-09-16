// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr, useUiPrefs, type LocalePref, type ThemePref } from '@typvia/ui';
import { Choice } from '../../paper/choice';
import { GroupTitle } from '../../paper/kit';
import { SettingRow } from './settings-kit';

/** Language and appearance: both take effect the moment they are chosen. */
export function LookGroups() {
  const tr = useTr();
  const { localePref, themePref, setLocalePref, setThemePref } = useUiPrefs();

  return (
    <>
      <section className="tvs-group">
        <GroupTitle>{tr('Language', '语言')}</GroupTitle>
        <SettingRow label={tr('Interface language', '界面语言')}>
          <Choice<LocalePref>
            label={tr('Interface language', '界面语言')}
            options={[
              { value: 'zh', label: '中文' },
              { value: 'en', label: 'English' },
              { value: 'system', label: tr('Follow the system', '跟随系统') },
            ]}
            value={localePref}
            onChange={setLocalePref}
          />
        </SettingRow>
      </section>

      <section className="tvs-group">
        <GroupTitle>{tr('Appearance', '外观')}</GroupTitle>
        <SettingRow label={tr('Light / dark', '亮 / 暗')}>
          <Choice<ThemePref>
            label={tr('Appearance', '外观')}
            options={[
              { value: 'system', label: tr('Follow the system', '跟随系统') },
              { value: 'light', label: tr('Light', '亮色') },
              { value: 'dark', label: tr('Dark', '暗色') },
            ]}
            value={themePref}
            onChange={setThemePref}
          />
        </SettingRow>
      </section>
    </>
  );
}

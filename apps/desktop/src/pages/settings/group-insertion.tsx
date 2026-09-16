// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { InjectionMethod } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { Choice } from '../../paper/choice';
import { GroupTitle } from '../../paper/kit';
import { useInsertMethod } from '../../workspace/insert-method';
import { Hint, SettingRow } from './settings-kit';

export function InsertionGroup() {
  const tr = useTr();
  const [method, setMethod] = useInsertMethod();

  return (
    <section className="tvs-group">
      <GroupTitle>{tr('Insertion', '插入')}</GroupTitle>
      <SettingRow label={tr('How it types', '怎么打出来')}>
        <Choice<InjectionMethod>
          label={tr('Insertion method', '插入方式')}
          options={[
            { value: 'keystrokes', label: tr('Type it out', '模拟键入') },
            { value: 'paste', label: tr('Paste from the clipboard', '剪贴板粘贴') },
          ]}
          value={method}
          onChange={setMethod}
        />
        <Hint>
          {method === 'keystrokes'
            ? tr(
                'Types the snippet into the app one character at a time, as if you typed it. Your clipboard is left alone; long snippets take a moment.',
                '像你自己打字一样,一个字一个字输进去。不动剪贴板,长片段会稍慢。',
              )
            : tr(
                'Puts the snippet on the clipboard and pastes it in one go. Whatever text you had copied is put back afterwards.',
                '先放进剪贴板再一次性粘贴,整段瞬间出现。你原来复制的文字随后放回去。',
              )}
        </Hint>
      </SettingRow>
    </section>
  );
}

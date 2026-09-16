// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { listen } from '@tauri-apps/api/event';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { KeyCap, TextAction } from '../../paper/kit';
import { StepBody, useEnterAdvances } from './step-kit';

/**
 * Step 4 — learned by doing. The host broadcasts a payload-free
 * `panel:summoned` when ⌘⇧V opens Quick Bar; only that real event says it
 * worked. The step can still be left without it.
 */
export function TryStep({ onFinish }: { onFinish: () => void }) {
  const tr = useTr();
  const [opened, setOpened] = useState(false);
  const [help, setHelp] = useState(false);

  useEffect(() => {
    const pending = listen('panel:summoned', () => setOpened(true));
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  useEnterAdvances(onFinish, opened);

  return (
    <StepBody
      mascot="typing"
      title={tr('Try it once.', '试着打一次。')}
      actions={
        <>
          <TextAction primary onClick={onFinish}>
            {tr('Start using Typvia', '开始用 Typvia')}
          </TextAction>
          {!help && (
            <TextAction onClick={() => setHelp(true)}>
              {tr('Nothing happened?', '没反应?')}
            </TextAction>
          )}
        </>
      }
    >
      <p>
        {tr(
          'Press ⌘⇧V in any app — Quick Bar floats over whatever you are doing. esc puts it away; then come back here.',
          '在任何应用里按 ⌘⇧V —— Quick Bar 会浮在你正在做的事上面。按 esc 收起,再回到这里。',
        )}
      </p>
      <div className="tvo-keys" aria-label="Command Shift V">
        <KeyCap>⌘</KeyCap>
        <KeyCap>⇧</KeyCap>
        <KeyCap>V</KeyCap>
      </div>
      <p className="tvo-said" role="status">
        {opened
          ? tr(
              'It came up just now. That is the whole gesture.',
              '刚才呼出来了。整个动作就是这样。',
            )
          : tr('Waiting for the press…', '等你按一下…')}
      </p>
      {help && (
        <p className="tvo-hint">
          {tr(
            'Opening Quick Bar needs no permission — if nothing appeared, another app may own ⌘⇧V. Inserting into other apps, and expanding triggers, needs Accessibility; without it inserts land on your clipboard to paste yourself.',
            '呼出 Quick Bar 不需要任何授权 —— 没出现的话,可能是别的应用占用了 ⌘⇧V。插入到别的应用、以及打触发词展开,需要辅助功能授权;没有它,插入的内容会放进剪贴板,由你自己粘贴。',
          )}
        </p>
      )}
    </StepBody>
  );
}

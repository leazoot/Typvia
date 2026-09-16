// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { TextAction } from '../../paper/kit';

/** Ask what the action should do — never open an empty form. */
export function NewAction({ onCreate }: { onCreate: (name: string, prompt: string) => void }) {
  const tr = useTr();
  const [text, setText] = useState('');
  const templates: Array<{ label: string; prompt: string }> = [
    {
      label: tr('Rewrite', '改写'),
      prompt: tr(
        'Rewrite the text below. Keep the meaning, make it clearer and shorter.',
        '改写下面的文本:保留原意,写得更清楚、更简短。',
      ),
    },
    {
      label: tr('Summarize', '摘要'),
      prompt: tr('Summarize the text below in three sentences.', '用三句话总结下面的文本。'),
    },
    {
      label: tr('Translate', '翻译'),
      prompt: tr(
        'Translate the text below into English. Keep names and code untouched.',
        '把下面的文本翻译成英文,保留人名与代码原样。',
      ),
    },
    {
      label: tr('Extract', '提取'),
      prompt: tr(
        'Extract every task from the text below as a short list.',
        '从下面的文本中提取所有待办事项,输出为简短列表。',
      ),
    },
    { label: tr('Custom', '自定义'), prompt: '' },
  ];

  return (
    <div className="tva-new">
      <h2>{tr('What should this action do?', '这个动作要做什么?')}</h2>
      <div className="tva-new-line">
        <input
          type="text"
          value={text}
          aria-label={tr('Describe the action', '描述这个动作')}
          placeholder={tr('Describe it…', '描述一下…')}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && text.trim() !== '') onCreate(text.trim(), '');
          }}
        />
        <TextAction primary disabled={text.trim() === ''} onClick={() => onCreate(text.trim(), '')}>
          {tr('Create', '创建')}
        </TextAction>
      </div>
      <div className="tva-templates">
        <span className="tpi-eyebrow">{tr('Start from', '从这里开始')}</span>
        {templates.map((template) => (
          <button
            key={template.label}
            type="button"
            className="tva-template"
            onClick={() => onCreate(template.label, template.prompt)}
          >
            {template.label}
          </button>
        ))}
      </div>
    </div>
  );
}

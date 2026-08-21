// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '../i18n';
import { TYPE_MARKS } from '../tokens';
import './type-mark.css';

/** Presentation vocabulary: core snippet-type strings → two-letter marks. */
const MARK_BY_TYPE: Record<string, string> = {
  text: 'TX',
  markdown: 'TX',
  code: 'CD',
  command: 'CM',
  prompt: 'PR',
  template: 'TP',
  sensitive: 'SC',
  ai_action: 'AI',
  link: 'LK',
  temporary: 'TX',
};

/** Two-letter mark for a snippet type; unknown values read as plain text. */
export function markForType(snippetType: string): string {
  return MARK_BY_TYPE[snippetType] ?? 'TX';
}

interface TypeMarkProps {
  /** One of the eight two-letter codes (TX/CD/CM/PR/TP/SC/AI/LK). */
  code: string;
}

/** Chinese full words for the marks; `TYPE_MARKS` (tokens) carries the English. */
const ZH_WORD_BY_CODE: Record<string, string> = {
  TX: '文本',
  CD: '代码',
  CM: '命令',
  PR: '提示词',
  TP: '模板',
  SC: '密钥',
  AI: 'AI 动作',
  LK: '链接',
};

/**
 * Two-letter mono type mark. Only `SC` (Secret) is accent-tinted; the rest
 * are neutral so the list survives greyscale. Screen readers get the full
 * word ("command"), never the letters.
 */
export function TypeMark({ code }: TypeMarkProps) {
  const tr = useTr();
  const word = TYPE_MARKS[code];
  const zhWord = ZH_WORD_BY_CODE[code];
  return (
    <span
      className={code === 'SC' ? 'tv-type-mark is-secret' : 'tv-type-mark'}
      role="img"
      aria-label={word !== undefined && zhWord !== undefined ? tr(word, zhWord) : (word ?? code)}
    >
      <span aria-hidden="true">{code}</span>
    </span>
  );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The provider kinds both hosts offer, with the consequences each choice
 * carries (default local URL, key requirement). The authoritative rules —
 * kind defaults, the URL egress policy — live in crates/ai; these entries
 * only word the choice.
 */
import type { Tr } from '../i18n';

export interface KindOption {
  id: string;
  label: string;
  labelZh: string;
  hint: string;
  hintZh: string;
  /** Whether the base URL can stay blank (a local default exists). */
  hasDefault: boolean;
  needsKey: boolean;
}

export const OLLAMA_KIND: KindOption = {
  id: 'ollama',
  label: 'Ollama',
  labelZh: 'Ollama',
  hint: 'Local Ollama — leave the URL blank for http://127.0.0.1:11434/v1.',
  hintZh: '本机 Ollama——URL 留空即使用 http://127.0.0.1:11434/v1。',
  hasDefault: true,
  needsKey: false,
};

export const PROVIDER_KINDS: KindOption[] = [
  OLLAMA_KIND,
  {
    id: 'lm_studio',
    label: 'LM Studio',
    labelZh: 'LM Studio',
    hint: 'Local LM Studio — leave the URL blank for http://127.0.0.1:1234/v1.',
    hintZh: '本机 LM Studio——URL 留空即使用 http://127.0.0.1:1234/v1。',
    hasDefault: true,
    needsKey: false,
  },
  {
    id: 'openai_compatible',
    label: 'OpenAI-compatible',
    labelZh: 'OpenAI 兼容',
    hint: 'A hosted service — base URL and your own API key required.',
    hintZh: '托管服务——需要 Base URL 和你自己的 API Key。',
    hasDefault: false,
    needsKey: true,
  },
  {
    id: 'custom_base_url',
    label: 'Custom URL',
    labelZh: '自定义 URL',
    hint: 'Any OpenAI-compatible endpoint you point at.',
    hintZh: '任意一个你指向的 OpenAI 兼容端点。',
    hasDefault: false,
    needsKey: false,
  },
];

export function kindLabel(kind: string, tr: Tr): string {
  const option = PROVIDER_KINDS.find((candidate) => candidate.id === kind);
  return option === undefined ? kind : tr(option.label, option.labelZh);
}

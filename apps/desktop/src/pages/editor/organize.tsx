// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { batchTagSnippets } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { suggestionFields, useOrganize } from '@typvia/ui/ai';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import type { Draft } from './use-editor-draft';

/**
 * Save-time organizing strip. The desktop editor has no dedicated design
 * for this surface; it borrows the share sheet's rule (fields pre-answered,
 * "the user's job is to confirm, not to fill in a form") in the editor's own
 * quiet language — a text control, no modal, no spinner, no icon. Nothing is
 * stored until a suggestion is explicitly applied; applying goes through the
 * editor's normal patch → debounced-save path.
 */

interface OrganizeProps {
  draft: Draft;
  /** The editor's patch — applying a field is the user's confirmation. */
  onApply: (changes: Partial<Draft>) => void;
}

export function Organize({ draft, onApply }: OrganizeProps) {
  const tr = useTr();
  const navigate = useNavigate();
  const { phase, run: runOrganize, reset } = useOrganize();
  const [tagsApplied, setTagsApplied] = useState(false);

  const run = async (providerId?: string) => {
    setTagsApplied(false);
    await runOrganize(
      { title: draft.title, body: draft.body, description: draft.description },
      providerId,
    );
  };

  if (phase.at === 'idle') {
    return (
      <div className="tv-ed-ai">
        <button
          type="button"
          className="tv-ed-ai-run"
          disabled={draft.body.trim() === ''}
          onClick={() => void run()}
        >
          {tr('Organize', 'AI 整理')}
        </button>
      </div>
    );
  }

  if (phase.at === 'loading') {
    return (
      <div className="tv-ed-ai" aria-busy="true">
        <span className="tv-ed-ai-note" role="status">
          {tr('Organizing…', '整理中…')}
        </span>
      </div>
    );
  }

  if (phase.at === 'unconfigured') {
    return (
      <div className="tv-ed-ai">
        <span className="tv-ed-ai-note" role="status">
          {tr(
            'Everything here works without AI — suggestions need a provider.',
            '不用 AI 一切照常可用——获取建议需要先配置提供方。',
          )}
        </span>
        <button type="button" className="tv-ed-ai-run" onClick={() => void navigate('/settings')}>
          {tr('Set up in Settings', '前往设置配置')}
        </button>
        <button type="button" className="tv-ed-ai-quiet" onClick={reset}>
          {tr('Dismiss', '忽略')}
        </button>
      </div>
    );
  }

  if (phase.at === 'error') {
    return (
      <div className="tv-ed-ai">
        <span className="tv-ed-ai-note" role="status">
          {tr(`Your draft is safe — ${phase.message}`, `草稿安全无损——${phase.message}`)}
        </span>
        <button type="button" className="tv-ed-ai-run" onClick={() => void run()}>
          {tr('Try again', '重试')}
        </button>
        <button type="button" className="tv-ed-ai-quiet" onClick={reset}>
          {tr('Dismiss', '忽略')}
        </button>
      </div>
    );
  }

  const { suggestion, providers, providerId } = phase;
  const fields = suggestionFields(suggestion, draft);

  const applyTags = () => {
    if (draft.id === null) return;
    const id = draft.id;
    Promise.all(suggestion.tags.map((tag) => batchTagSnippets([id], tag.id)))
      .then(() => setTagsApplied(true))
      .catch(() => {
        // Tags are additive and retryable; the row stays offered.
      });
  };

  const empty = fields.length === 0 && suggestion.tags.length === 0;

  return (
    <div className="tv-ed-ai" role="group" aria-label={tr('AI suggestions', 'AI 建议')}>
      {empty ? (
        <span className="tv-ed-ai-note" role="status">
          {tr(
            'No suggestions — this draft already looks organized.',
            '暂无建议——这份草稿看起来已经整理好了。',
          )}
        </span>
      ) : (
        <>
          <span className="tv-ed-ai-note" role="status">
            {tr('Suggestions', '建议')}
          </span>
          {fields.map((field) => (
            <button
              key={field.key}
              type="button"
              className="tv-ed-ai-field"
              onClick={() => onApply(field.changes)}
            >
              <span className="tv-ed-ai-field-label">{tr(field.label, field.labelZh)}</span>
              <span className="tv-ed-ai-field-value">{field.value}</span>
            </button>
          ))}
          {suggestion.tags.length > 0 && draft.id !== null && (
            <button
              type="button"
              className="tv-ed-ai-field"
              disabled={tagsApplied}
              onClick={applyTags}
            >
              <span className="tv-ed-ai-field-label">{tr('Tags', '标签')}</span>
              <span className="tv-ed-ai-field-value">
                {tagsApplied
                  ? tr('Applied', '已应用')
                  : suggestion.tags.map((tag) => tag.name).join(', ')}
              </span>
            </button>
          )}
          {fields.length > 1 && (
            <button
              type="button"
              className="tv-ed-ai-run"
              onClick={() => {
                onApply(
                  fields.reduce<Partial<Draft>>(
                    (merged, field) => ({ ...merged, ...field.changes }),
                    {},
                  ),
                );
              }}
            >
              {tr('Apply all', '全部应用')}
            </button>
          )}
        </>
      )}
      {suggestion.securityLevel === 'sensitive' && (
        <span className="tv-ed-ai-note">
          {tr(
            'Looks sensitive — consider the Security tab, which encrypts it.',
            '内容可能敏感——可考虑使用「安全」标签页加密。',
          )}
        </span>
      )}
      {providers.length > 1 && (
        <select
          className="tv-ed-ai-provider"
          aria-label={tr('AI provider', 'AI 提供方')}
          value={providerId}
          onChange={(event) => void run(event.target.value)}
        >
          {providers.map((provider) => (
            <option key={provider.id} value={provider.id}>
              {provider.name}
            </option>
          ))}
        </select>
      )}
      <button type="button" className="tv-ed-ai-quiet" onClick={reset}>
        {tr('Dismiss', '忽略')}
      </button>
    </div>
  );
}

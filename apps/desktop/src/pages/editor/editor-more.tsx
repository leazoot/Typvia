// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Folder } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import type { ReactNode } from 'react';
import { Choice } from '../../paper/choice';
import { CheckBox, GroupTitle } from '../../paper/kit';
import type { Draft } from './use-editor-draft';
import { VaultMove } from './vault-move';

const TRIGGER_MODES: ReadonlyArray<{ value: string; label: readonly [string, string] }> = [
  { value: 'delimiter', label: ['After a space or punctuation', '打完空格或标点'] },
  { value: 'immediate', label: ['At once', '立刻'] },
  { value: 'word_boundary', label: ['At a word boundary', '词边界'] },
  { value: 'regex', label: ['Regular expression', '正则表达式'] },
];

/** Editable kinds — `sensitive` is the vault's, `temporary` is not offered. */
const KINDS: ReadonlyArray<{ value: string; label: readonly [string, string] }> = [
  { value: 'text', label: ['Text', '文本'] },
  { value: 'markdown', label: ['Markdown', 'Markdown'] },
  { value: 'code', label: ['Code', '代码'] },
  { value: 'command', label: ['Command', '命令'] },
  { value: 'prompt', label: ['Prompt', '提示词'] },
  { value: 'template', label: ['Template', '模板'] },
  { value: 'ai_action', label: ['AI action', 'AI 操作'] },
  { value: 'link', label: ['Link', '链接'] },
];

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="tve-row">
      <span className="tve-row-label">{label}</span>
      <div className="tve-row-value">{children}</div>
    </div>
  );
}

/**
 * Everything else a snippet has, below the variables: how it expands, where
 * it lives, whether it is on, and how to keep it secret. Each fact sits in the 118px label column; choices are trace options.
 */
export function EditorMore({
  draft,
  onPatch,
  folders,
  sensitiveKinds,
  saved,
}: {
  draft: Draft;
  onPatch: (changes: Partial<Draft>) => void;
  folders: readonly Folder[];
  sensitiveKinds: readonly string[];
  saved: boolean;
}) {
  const tr = useTr();

  return (
    <section className="tve-more" aria-label={tr('More about this snippet', '这条片段的其余设置')}>
      <GroupTitle>{tr('More', '更多')}</GroupTitle>
      <div className="tve-rows">
        <Row label={tr('Expands', '展开时机')}>
          {draft.trigger === null ? (
            <span className="tve-muted">{tr('Set a trigger first', '先设触发词')}</span>
          ) : (
            <Choice
              label={tr('Expands', '展开时机')}
              value={draft.triggerMode ?? 'delimiter'}
              onChange={(triggerMode) => onPatch({ triggerMode })}
              options={TRIGGER_MODES.map((mode) => ({
                value: mode.value,
                label: tr(...mode.label),
              }))}
            />
          )}
        </Row>
        <Row label={tr('Collection', '集合')}>
          <Choice
            label={tr('Collection', '集合')}
            value={draft.folderId ?? ''}
            onChange={(id) => onPatch({ folderId: id === '' ? null : id })}
            options={[
              ...folders.map((folder) => ({ value: folder.id, label: folder.name })),
              { value: '', label: tr('Unsorted', '待整理') },
            ]}
          />
        </Row>
        <Row label={tr('Kind', '类型')}>
          <Choice
            label={tr('Kind', '类型')}
            value={draft.snippetType}
            onChange={(snippetType) => onPatch({ snippetType })}
            options={KINDS.map((kind) => ({ value: kind.value, label: tr(...kind.label) }))}
          />
        </Row>
        <Row label={tr('Description', '描述')}>
          <span className="tve-line is-small">
            <input
              type="text"
              aria-label={tr('Description', '描述')}
              placeholder={tr('Optional — only you see it', '可以不填 —— 只有你看得到')}
              value={draft.description ?? ''}
              onChange={(event) => {
                const value = event.target.value;
                onPatch({ description: value === '' ? null : value });
              }}
            />
          </span>
        </Row>
        <Row label={tr('Language', '语言')}>
          <span className="tve-line is-small is-mono">
            <input
              type="text"
              spellCheck={false}
              aria-label={tr('Language', '语言')}
              placeholder={tr('e.g. sql', '例如 sql')}
              value={draft.language ?? ''}
              onChange={(event) => {
                const value = event.target.value;
                onPatch({ language: value === '' ? null : value });
              }}
            />
          </span>
        </Row>
        <Row label={tr('State', '状态')}>
          <span className="tve-checks">
            <CheckBox checked={draft.isEnabled} onChange={(isEnabled) => onPatch({ isEnabled })}>
              {tr('Enabled', '启用')}
            </CheckBox>
            <CheckBox checked={draft.isPinned} onChange={(isPinned) => onPatch({ isPinned })}>
              {tr('Pinned', '置顶')}
            </CheckBox>
          </span>
        </Row>
      </div>

      <GroupTitle>{tr('Keeping it secret', '保密')}</GroupTitle>
      <VaultMove draft={draft} sensitiveKinds={sensitiveKinds} saved={saved} />
    </section>
  );
}

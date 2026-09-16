// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { TemplateField, TemplateFieldType } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { GroupTitle, TextAction } from '../../paper/kit';
import { EditorBody } from './editor-body';
import { Organize } from './organize';
import type { Draft, SaveStatus } from './use-editor-draft';

/**
 * Trigger and title: each one line under the words, no box. The hint under
 * each says what it is for — and a failed save says why, in the same place.
 */
export function LineFields({
  draft,
  onPatch,
  status,
  errorMessage,
  focusTrigger,
}: {
  draft: Draft;
  onPatch: (changes: Partial<Draft>) => void;
  status: SaveStatus;
  errorMessage: string | null;
  focusTrigger: boolean;
}) {
  const tr = useTr();
  const triggerInput = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (focusTrigger) triggerInput.current?.focus();
  }, [focusTrigger]);

  const trigger = draft.trigger ?? '';
  const length = String([...trigger].length);
  // The "no clash" claim follows the trigger that last saved, not the save
  // status: other edits (expansion mode, kind) autosave too, and tying the
  // sentence to status made it blink out and back on every one of them.
  const [savedTrigger, setSavedTrigger] = useState<string | null>(
    status === 'saved' ? trigger : null,
  );
  if (status === 'saved' && savedTrigger !== trigger) setSavedTrigger(trigger);
  const unique = errorMessage === null && savedTrigger === trigger;
  const triggerHint =
    errorMessage ??
    (trigger === ''
      ? tr(
          'No trigger is fine too — ⌘⇧V still finds it by its words.',
          '不设触发词也行 —— ⌘⇧V 里照样按字找得到。',
        )
      : tr(
          `Type these ${length} characters and it expands.${unique ? ' No other snippet uses them.' : ''}`,
          `打完这 ${length} 个字符就展开。${unique ? '没有和别的片段撞。' : ''}`,
        ));

  return (
    <div className="tve-lines">
      <div className="tve-line-field is-trigger">
        <span className="tve-caps">{tr('Trigger', '触发词')}</span>
        <span className="tve-line">
          <input
            ref={triggerInput}
            type="text"
            spellCheck={false}
            aria-label={tr('Trigger', '触发词')}
            placeholder="/mtg"
            value={trigger}
            onChange={(event) => {
              const value = event.target.value;
              onPatch({ trigger: value === '' ? null : value });
            }}
          />
        </span>
        <span
          className={errorMessage === null ? 'tve-line-hint' : 'tve-line-hint is-error'}
          role={errorMessage === null ? undefined : 'alert'}
        >
          {triggerHint}
        </span>
      </div>
      <div className="tve-line-field is-title">
        <span className="tve-caps">{tr('Title', '标题')}</span>
        <span className="tve-line">
          <input
            type="text"
            aria-label={tr('Snippet title', '片段标题')}
            placeholder={tr('Untitled snippet', '未命名片段')}
            value={draft.title}
            onChange={(event) => onPatch({ title: event.target.value })}
          />
        </span>
        <span className="tve-line-hint">
          {tr(
            'Only for finding it in the library; it is never inserted.',
            '只在资料库里用来找它,插入时不带出去。',
          )}
        </span>
      </div>
    </div>
  );
}

/**
 * The body is the one boxed input, because it is the one that runs to many lines.
 * Organizing sits on the line beneath it, since the body is what it reads.
 */
export function BodyBlock({
  draft,
  onPatch,
  variables,
  code,
}: {
  draft: Draft;
  onPatch: (changes: Partial<Draft>) => void;
  variables: readonly string[];
  code: boolean;
}) {
  const tr = useTr();
  const n = String(variables.length);
  return (
    <>
      <div className="tve-group">
        <GroupTitle>{tr('Body', '正文')}</GroupTitle>
      </div>
      <div className={code ? 'tve-body-card is-code' : 'tve-body-card'}>
        <EditorBody value={draft.body} onChange={(body) => onPatch({ body })} />
      </div>
      <div className="tve-body-foot">
        <p className="tve-var-line">
          {variables.length === 0
            ? tr(
                'Type {{name}} anywhere to leave a blank that is filled in when inserting.',
                '在任意位置输入 {{名字}},就留下一个插入时再填的空。',
              )
            : tr(
                `${n} variables. Tab through them when inserting; any left empty stay empty.`,
                `${n} 个变量。插入时按 ⇥ 逐个填,不填就留空。`,
              )}
        </p>
        <Organize draft={draft} onApply={onPatch} />
      </div>
    </>
  );
}

const FIELD_TYPE_WORDS: Record<TemplateFieldType, readonly [string, string]> = {
  single_line_text: ['One line of text', '单行文字'],
  multi_line_text: ['Several lines of text', '多行文字'],
  number: ['A number', '数字'],
  date: ['A date', '日期'],
  time: ['A time', '时间'],
  single_select: ['One of a list', '单选'],
  multi_select: ['Some of a list', '多选'],
  toggle: ['On or off', '开关'],
  dropdown: ['One of a menu', '下拉选择'],
  dynamic_variable: ['Filled in by itself', '自动填入'],
  secret_ref: ['A vault reference', '保险库引用'],
};

function describe(field: TemplateField | undefined, tr: Tr): string {
  if (field === undefined) return '';
  const parts = [tr(...FIELD_TYPE_WORDS[field.fieldType])];
  if (field.isRequired) parts.push(tr('must be filled', '必须填'));
  return parts.join(' · ');
}

/** Each variable on its own line, in the 118px label column: what it will hold by default. */
export function VariableDefs({
  names,
  fields,
  canOpenBuilder,
  onOpenBuilder,
}: {
  names: readonly string[];
  fields: readonly TemplateField[];
  canOpenBuilder: boolean;
  onOpenBuilder: () => void;
}) {
  const tr = useTr();
  if (names.length === 0) return null;
  return (
    <div className="tve-vars">
      {names.map((name) => {
        const field = fields.find((entry) => entry.name === name);
        const fallback = field?.defaultValue ?? '';
        const detail = describe(field, tr);
        return (
          <div key={name} className="tve-var">
            <span className="tve-var-name">{`{{${name}}}`}</span>
            <span className="tve-var-text">
              <span className="tve-var-value">
                {fallback === ''
                  ? tr('Left empty by default', '默认留空')
                  : tr(`Defaults to “${fallback}”`, `默认填「${fallback}」`)}
              </span>
              {detail !== '' && <span className="tve-var-detail">{detail}</span>}
            </span>
          </div>
        );
      })}
      {canOpenBuilder && (
        <TextAction onClick={onOpenBuilder}>
          {tr('Set defaults and kinds in the template builder', '在模板编辑器里设默认值和类型')}
        </TextAction>
      )}
    </div>
  );
}

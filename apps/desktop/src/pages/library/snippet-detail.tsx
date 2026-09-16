// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { openAccessibilitySettings, type Snippet } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { Note, TextAction } from '../../paper/kit';
import { useInsertMethod } from '../../workspace/insert-method';
import { DetailFill } from './detail-fill';
import { isCode } from './library-model';

export type InsertFailure = 'permission' | 'rule' | 'other';

function variablesValue(variables: readonly string[] | undefined, tr: Tr): string {
  if (variables === undefined) return '—';
  if (variables.length === 0) return tr('No variables', '没有变量');
  const names = variables.map((name) => `{{${name}}}`).join(' ');
  const count = String(variables.length);
  return tr(`${count} · ${names}`, `${count} 个 · ${names}`);
}

/** What went wrong says first that nothing was lost, then offers a way on. */
function FailureNote({
  failure,
  total,
  onCopy,
}: {
  failure: InsertFailure;
  total: number;
  onCopy: () => void;
}) {
  const tr = useTr();
  const n = total.toLocaleString('en-US');
  const [title, body] =
    failure === 'permission'
      ? [
          tr("Typvia can't type for you yet.", 'Typvia 还不能替你打字。'),
          tr(
            `macOS wants Typvia ticked under System Settings → Privacy & Security → Accessibility. No restart once it is. All ${n} snippets are saved; not one is lost.`,
            `macOS 要你在「系统设置 → 隐私与安全性 → 辅助功能」里给 Typvia 打上勾。打完不用重启,回来就能用。${n} 条片段已经存好了,一条都没丢。`,
          ),
        ]
      : failure === 'rule'
        ? [
            tr("That app doesn't take this one.", '那个应用不收这一条。'),
            tr(
              'A rule you set hides it there. The snippet itself is unchanged.',
              '你给那个应用定过规则,这条在那里是隐藏的。片段本身没有变。',
            ),
          ]
        : [
            tr("It didn't go in this time.", '这次没插进去。'),
            tr('The snippet is unchanged; you can try again.', '片段没有变,可以再试一次。'),
          ];
  return (
    <Note label={title}>
      <p className="tvl-note-title">{title}</p>
      <p className="tvl-note-body">{body}</p>
      {failure === 'permission' && (
        <div className="tvl-note-actions">
          <TextAction
            primary
            onClick={() => void openAccessibilitySettings().catch(() => undefined)}
          >
            {tr('Open that page', '打开那一页')}
          </TextAction>
          <TextAction onClick={onCopy}>
            {tr('Just copy it to the clipboard for now', '先只用「复制到剪贴板」')}
          </TextAction>
        </div>
      )}
    </Note>
  );
}

/**
 * The snippet in full: the body at reading size, its facts in this pane's
 * narrower label column, and the actions as words. Below 900px it becomes a
 * drawer.
 */
export function SnippetDetail({
  snippet,
  collection,
  variables,
  used,
  sync,
  total,
  failure,
  open,
  filling,
  onInsert,
  onFill,
  onFillCancel,
  onInsertFilled,
  onCopy,
  onEdit,
  onClose,
}: {
  snippet: Snippet | undefined;
  collection: string;
  variables: readonly string[] | undefined;
  used: string;
  sync: string;
  total: number;
  failure: InsertFailure | null;
  open: boolean;
  /** The template's variables are being filled in here. */
  filling: boolean;
  onInsert: () => void;
  onFill: () => void;
  onFillCancel: () => void;
  onInsertFilled: (values: Record<string, string>) => void;
  onCopy: () => void;
  onEdit: () => void;
  onClose: () => void;
}) {
  const tr = useTr();
  const [method] = useInsertMethod();
  if (snippet === undefined) return <section className="tvl-detail" data-open="false" />;
  const hasVariables = variables !== undefined && variables.length > 0;
  const fields: Array<[string, string, boolean]> = [
    [tr('Trigger', '触发词'), snippet.trigger ?? tr('None', '没有'), snippet.trigger !== null],
    [tr('Collection', '集合'), collection, false],
    [tr('Variables', '变量'), variablesValue(variables, tr), false],
    [tr('Used', '用过'), used, false],
    [
      tr('Insert by', '插入方式'),
      method === 'keystrokes' ? tr('Type it out', '模拟键入') : tr('Clipboard paste', '剪贴板粘贴'),
      false,
    ],
    [tr('Sync', '同步'), sync, false],
  ];

  return (
    <section className="tvl-detail" data-open={open} aria-label={tr('Snippet', '片段')}>
      <button type="button" className="tvl-detail-close" onClick={onClose}>
        {tr('Close', '收起')}
      </button>
      <div className="tpi-eyebrow tvl-detail-eyebrow">
        {snippet.trigger === null ? collection : `${collection} · ${snippet.trigger}`}
      </div>
      <div className={isCode(snippet) ? 'tvl-detail-body is-code' : 'tvl-detail-body'}>
        {snippet.body ?? snippet.title}
      </div>
      {filling ? (
        <DetailFill snippet={snippet} onInsert={onInsertFilled} onCancel={onFillCancel} />
      ) : (
        hasVariables && (
          <p className="tvl-detail-note">
            {tr(
              'Its variables are filled in before it goes in: choose “Fill variables, then insert” or press ⏎.',
              '带变量的片段插入前先逐格填好:点「填变量后插入」,或者按 ⏎。',
            )}
          </p>
        )
      )}
      <div className="tvl-detail-rule" />
      <dl className="tvl-fields">
        {fields.map(([label, value, mono]) => (
          <div key={label} className="tvl-field">
            <dt>{label}</dt>
            <dd className={mono ? 'is-mono' : undefined}>{value}</dd>
          </div>
        ))}
      </dl>
      <div className="tvl-grow" />
      {failure !== null && (
        <div className="tvl-detail-failure">
          <FailureNote failure={failure} total={total} onCopy={onCopy} />
        </div>
      )}
      <div className="tvl-detail-actions" hidden={filling}>
        {hasVariables ? (
          <>
            <TextAction primary onClick={onFill}>
              {tr('Fill variables, then insert', '填变量后插入')}
            </TextAction>
            <TextAction onClick={onEdit}>{tr('Edit', '编辑')}</TextAction>
            <span className="tvl-grow" />
            <span className="tvl-detail-hint">{tr('⏎ fills too', '⏎ 也能填')}</span>
          </>
        ) : (
          <>
            <TextAction primary onClick={onInsert}>
              {tr('Insert into the front app', '插入到最前应用')}
            </TextAction>
            <TextAction onClick={onEdit}>{tr('Edit', '编辑')}</TextAction>
            <span className="tvl-grow" />
            <span className="tvl-detail-hint">{tr('⏎ inserts too', '⏎ 也能插入')}</span>
          </>
        )}
      </div>
    </section>
  );
}

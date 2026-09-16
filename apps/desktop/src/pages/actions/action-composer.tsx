// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  aiActionDelete,
  type AiActionInputSource,
  type AiActionOutputMode,
  type AiActionPermissionScope,
  type AiProvider,
  createSnippet,
} from '@typvia/shared';
import { confirmLabel, modeLabel, scopeLabel, sourceLabel, useActionRun } from '@typvia/ui/ai';
import { useTr, type Tr } from '@typvia/ui';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { Choice } from '../../paper/choice';
import { TextAction } from '../../paper/kit';
import { usePaperMenu } from '../../paper/menu';
import type { ActionDraft } from './action-draft';

const SOURCE_KEYS: AiActionInputSource[] = ['selection', 'clipboard', 'snippet', 'share'];
const MODE_KEYS: AiActionOutputMode[] = ['replace', 'insert', 'copy', 'new_snippet'];
const SCOPE_KEYS: AiActionPermissionScope[] = ['normal_only', 'mask_secrets'];
/** Quiet, useful temperatures; the blank one means "the provider decides". */
const TEMPERATURES = ['', '0.2', '0.5', '0.7', '1.0'];

export type SaveState = 'idle' | 'saving' | 'saved' | 'error';

/** Short words for the flow sentence — the long ones live in the popovers. */
function shortSource(source: AiActionInputSource, tr: Tr): string {
  const labels: Record<AiActionInputSource, string> = {
    selection: tr('Selection', '选中内容'),
    clipboard: tr('Clipboard', '剪贴板'),
    snippet: tr('Snippet', '片段'),
    share: tr('Shared text', '分享文本'),
  };
  return labels[source];
}

function shortMode(mode: AiActionOutputMode, tr: Tr): string {
  const labels: Record<AiActionOutputMode, string> = {
    replace: tr('Confirm → replace', '确认 → 替换'),
    insert: tr('Confirm → insert', '确认 → 插入'),
    copy: tr('Confirm → copy', '确认 → 复制'),
    new_snippet: tr('Confirm → new snippet', '确认 → 新建片段'),
  };
  return labels[mode];
}

/**
 * One action, laid out as a sentence and a page: the flow in underlined words
 * that open their choices, the instruction as the one boxed field, privacy and
 * the technical settings folded behind their label column, and the test bench
 * folded until asked for. It saves itself.
 */
export function ActionComposer({
  draft,
  providers,
  saveState,
  onEdit,
  onDuplicate,
  onDeleted,
}: {
  draft: ActionDraft;
  providers: AiProvider[];
  saveState: SaveState;
  onEdit: (next: ActionDraft) => void;
  onDuplicate: () => void;
  onDeleted: () => void;
}) {
  const tr = useTr();
  const navigate = useNavigate();
  const menu = usePaperMenu(tr('Action options', '动作选项'));
  const [renaming, setRenaming] = useState(false);
  const [privacyOpen, setPrivacyOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [benchOpen, setBenchOpen] = useState(false);
  const [testText, setTestText] = useState('');
  const { run, start, applied, failed, dismiss } = useActionRun();

  const provider = providers.find((entry) => entry.id === draft.providerId) ?? null;
  const modelWord =
    draft.model.trim() !== ''
      ? draft.model.trim()
      : (provider?.model ?? tr('no provider', '未选 Provider'));

  const runAction = async () => {
    if (draft.id === null || testText.trim() === '') return;
    await start(draft.id, {
      text: testText,
      source: draft.inputSource,
      // The bench holds typed or pasted text, never vault content.
      isSensitive: false,
    });
  };

  const confirm = async (output: string) => {
    switch (draft.outputMode) {
      case 'replace':
        setTestText(output);
        applied(tr('Input replaced.', '已替换输入。'));
        return;
      case 'insert':
        setTestText(`${testText}\n\n${output}`);
        applied(tr('Inserted below the input.', '已插入到输入下方。'));
        return;
      case 'copy':
        await navigator.clipboard.writeText(output);
        applied(tr('Copied to the clipboard.', '已复制到剪贴板。'));
        return;
      case 'new_snippet': {
        try {
          await createSnippet({ title: draft.name, body: output, snippetType: 'text' });
          applied(tr('Saved to your library as a snippet.', '已作为片段保存到你的片段库。'));
        } catch {
          failed(
            tr(
              'the snippet could not be saved — the result is above',
              '片段未能保存——结果仍显示在上方',
            ),
          );
        }
        return;
      }
    }
  };

  const openMenu = (button: HTMLElement) => {
    const rect = button.getBoundingClientRect();
    menu.open({ x: rect.right - 180, y: rect.bottom + 4 }, [
      { kind: 'item', label: tr('Rename', '重命名'), onSelect: () => setRenaming(true) },
      { kind: 'item', label: tr('Duplicate', '创建副本'), onSelect: onDuplicate },
      { kind: 'separator' },
      {
        kind: 'item',
        label: tr('Delete', '删除'),
        danger: true,
        onSelect: () => {
          if (draft.id !== null) void aiActionDelete(draft.id).then(onDeleted);
        },
      },
    ]);
  };

  return (
    <div className="tva-composer-sheet">
      <div className="tva-title-row">
        {renaming ? (
          <input
            // Renaming is a moment, and the field should already be live.
            autoFocus
            className="tva-title-input"
            type="text"
            aria-label={tr('Action name', '动作名称')}
            value={draft.name}
            onChange={(event) => onEdit({ ...draft, name: event.target.value })}
            onBlur={() => setRenaming(false)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === 'Escape') setRenaming(false);
            }}
          />
        ) : (
          <button type="button" className="tva-title" onClick={() => setRenaming(true)}>
            {draft.name === '' ? tr('Untitled action', '未命名动作') : draft.name}
          </button>
        )}
        {draft.isBuiltin && <span className="tva-note">{tr('Built-in', '内置')}</span>}
        <span className={`tva-save ${saveState === 'idle' ? 'is-quiet' : ''}`} role="status">
          {saveState === 'saving'
            ? tr('Saving…', '保存中…')
            : saveState === 'error'
              ? tr('Not saved — try again', '没存上——再试一次')
              : tr('Saved', '已保存')}
        </span>
        <button
          type="button"
          className="tva-more"
          aria-label={tr('Action options', '动作选项')}
          aria-haspopup="menu"
          onClick={(event) => openMenu(event.currentTarget)}
        >
          ···
        </button>
      </div>

      {/* The flow sentence: what comes in, what thinks, where it goes. */}
      <div className="tva-flow" aria-label={tr('Flow', '数据流')}>
        <TokenMenu label={shortSource(draft.inputSource, tr)} title={tr('Input', '输入')}>
          {(close) =>
            SOURCE_KEYS.map((key) => (
              <button
                key={key}
                type="button"
                className="tva-pop-item"
                aria-current={key === draft.inputSource}
                onClick={() => {
                  close();
                  onEdit({ ...draft, inputSource: key });
                }}
              >
                {sourceLabel(key, tr)}
              </button>
            ))
          }
        </TokenMenu>
        <span aria-hidden="true" className="tva-arrow">
          →
        </span>
        <TokenMenu
          label={provider === null ? modelWord : `${provider.name} / ${modelWord}`}
          title={tr('Model', '模型')}
          running={run.at === 'running'}
        >
          {(close) => (
            <>
              {providers.map((entry) => (
                <button
                  key={entry.id}
                  type="button"
                  className="tva-pop-item"
                  aria-current={entry.id === draft.providerId}
                  onClick={() => {
                    close();
                    onEdit({ ...draft, providerId: entry.id, model: '' });
                  }}
                >
                  {entry.name} · {entry.model}
                </button>
              ))}
              <hr />
              <input
                className="tva-pop-input"
                type="text"
                aria-label={tr('Model override', '模型覆盖')}
                placeholder={tr('Model override…', '指定其他模型…')}
                value={draft.model}
                onChange={(event) => onEdit({ ...draft, model: event.target.value })}
              />
            </>
          )}
        </TokenMenu>
        <span aria-hidden="true" className="tva-arrow">
          →
        </span>
        <TokenMenu label={shortMode(draft.outputMode, tr)} title={tr('Output', '输出')}>
          {(close) =>
            MODE_KEYS.map((key) => (
              <button
                key={key}
                type="button"
                className="tva-pop-item"
                aria-current={key === draft.outputMode}
                onClick={() => {
                  close();
                  onEdit({ ...draft, outputMode: key });
                }}
              >
                {modeLabel(key, tr)}
              </button>
            ))
          }
        </TokenMenu>
      </div>

      <div className="tva-prompt">
        <textarea
          aria-label={tr('Instruction', '指令')}
          placeholder={tr(
            'Tell the model what to do with the text…',
            '告诉模型要对这段文本做什么…',
          )}
          value={draft.promptTemplate}
          onChange={(event) => onEdit({ ...draft, promptTemplate: event.target.value })}
        />
        <span className="tva-prompt-count">
          {tr(
            `${String(draft.promptTemplate.length)} chars`,
            `${String(draft.promptTemplate.length)} 字`,
          )}
        </span>
      </div>

      <div className="tva-rows">
        <button
          type="button"
          className="tva-setting"
          aria-expanded={privacyOpen}
          onClick={() => setPrivacyOpen((open) => !open)}
        >
          <span className="tva-setting-label">{tr('Privacy', '隐私')}</span>
          <span className="tva-setting-value">
            {scopeLabel(draft.permissionScope, tr)}
            <span aria-hidden="true" className="tva-chev">
              ›
            </span>
          </span>
        </button>
        <div className={privacyOpen ? 'tva-fold is-open' : 'tva-fold'}>
          <div>
            <div className="tva-fold-inner">
              <span className="tpi-eyebrow">{tr('Sensitive content', '敏感内容')}</span>
              <Choice<AiActionPermissionScope>
                label={tr('Sensitive content', '敏感内容')}
                options={SCOPE_KEYS.map((key) => ({ value: key, label: scopeLabel(key, tr) }))}
                value={draft.permissionScope}
                onChange={(key) => onEdit({ ...draft, permissionScope: key })}
              />
              <p className="tva-note">
                {tr(
                  'Vault contents are never sent, whichever you pick.',
                  '无论选哪一项,保险库内容永远不会被发送。',
                )}
              </p>
            </div>
          </div>
        </div>

        <button
          type="button"
          className="tva-setting"
          aria-expanded={advancedOpen}
          onClick={() => setAdvancedOpen((open) => !open)}
        >
          <span className="tva-setting-label">{tr('Advanced', '高级')}</span>
          <span className="tva-setting-value">
            <span aria-hidden="true" className="tva-chev">
              ›
            </span>
          </span>
        </button>
        <div className={advancedOpen ? 'tva-fold is-open' : 'tva-fold'}>
          <div>
            <div className="tva-fold-inner">
              <span className="tpi-eyebrow">{tr('Temperature', '温度')}</span>
              <Choice
                label={tr('Temperature', '温度')}
                options={TEMPERATURES.map((value) => ({
                  value,
                  label: value === '' ? tr('Provider default', 'Provider 默认') : value,
                }))}
                value={draft.temperature}
                onChange={(value) => onEdit({ ...draft, temperature: value })}
              />
              <p className="tva-note">
                {tr(
                  'The input text is sent after the instruction; every request is written to the outbound log.',
                  '输入文本会附在指令之后发送;每次请求都会记入出站日志。',
                )}
              </p>
            </div>
          </div>
        </div>
      </div>

      <div className="tva-bench">
        <button
          type="button"
          className="tva-bench-toggle"
          aria-expanded={benchOpen}
          onClick={() => setBenchOpen((open) => !open)}
        >
          <span aria-hidden="true" className="tva-chev">
            ›
          </span>
          {tr('Test action', '试运行')}
        </button>
        {benchOpen && (
          <div className="tva-bench-body">
            <span className="tpi-eyebrow">{tr('Input', '输入')}</span>
            <textarea
              aria-label={tr('Test input', '试运行输入')}
              placeholder={tr('Paste something here…', '在这里粘贴一段文本…')}
              value={testText}
              onChange={(event) => setTestText(event.target.value)}
              onKeyDown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
                  event.preventDefault();
                  void runAction();
                }
              }}
            />
            <div className="tva-bench-bar">
              <TextAction
                primary
                disabled={draft.id === null || testText.trim() === '' || run.at === 'running'}
                onClick={() => void runAction()}
              >
                {tr('Run  ⌘↵', '运行  ⌘↵')}
              </TextAction>
              {draft.id === null && (
                <span className="tva-note">
                  {tr(
                    'Name the action first — it saves itself.',
                    '先给动作起个名字,它会自动保存。',
                  )}
                </span>
              )}
            </div>

            {run.at === 'error' && (
              <div className="tpi-note tva-inline-error" role="status">
                <p className="tpi-note-title">
                  {tr('The provider did not answer', 'Provider 没有响应')}
                </p>
                <p className="tpi-note-body">
                  {tr(
                    `Your text is unchanged — ${run.message}`,
                    `你的文本没有改动——${run.message}`,
                  )}
                </p>
                <div className="tpi-note-actions">
                  <TextAction primary onClick={() => void runAction()}>
                    {tr('Retry', '重试')}
                  </TextAction>
                  <TextAction onClick={() => void navigate('/settings')}>
                    {tr('Provider settings', 'Provider 设置')}
                  </TextAction>
                </div>
              </div>
            )}

            {run.at === 'pending' && (
              <>
                <span className="tpi-eyebrow">{tr('Output', '输出')}</span>
                {run.maskedKinds.length > 0 && (
                  <p className="tva-warn" role="status">
                    {tr(
                      `Secrets stripped before sending · ${run.maskedKinds.join(', ')}`,
                      `发送前已剔除 Secret · ${run.maskedKinds.join(', ')}`,
                    )}
                  </p>
                )}
                <pre className="tva-output">{run.output}</pre>
                <div className="tva-bench-bar">
                  <TextAction primary onClick={() => void confirm(run.output)}>
                    {`↵ ${confirmLabel(draft.outputMode, tr)}`}
                  </TextAction>
                  <TextAction onClick={() => void runAction()}>
                    {tr('Run again', '再次运行')}
                  </TextAction>
                  <TextAction onClick={dismiss}>{tr('Dismiss', '关闭')}</TextAction>
                  <span className="tva-note">
                    {tr(
                      `${String(run.words)} words left this Mac · 1 request · ${run.seconds}s`,
                      `${String(run.words)} 个词离开了本机 · 1 次请求 · ${run.seconds} 秒`,
                    )}
                  </span>
                </div>
              </>
            )}

            {run.at === 'applied' && (
              <p className="tva-note" role="status" aria-live="assertive">
                {run.note}
              </p>
            )}
          </div>
        )}
      </div>
      {menu.node}
    </div>
  );
}

/** A word in the flow sentence, underlined, that opens its choices below it. */
function TokenMenu({
  label,
  title,
  running = false,
  children,
}: {
  label: string;
  title: string;
  running?: boolean;
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!open) return;
    const away = (event: MouseEvent) => {
      if (!(event.target instanceof Node) || wrap.current?.contains(event.target) !== true) {
        setOpen(false);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', away);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', away);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);

  return (
    <span ref={wrap} className="tva-token-wrap">
      <button
        type="button"
        className={running ? 'tva-token is-running' : 'tva-token'}
        aria-label={`${title}: ${label}`}
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        {label}
      </button>
      {open && (
        <span className="tva-pop" role="menu" aria-label={title}>
          <span className="tpi-eyebrow">{title}</span>
          {children(() => setOpen(false))}
        </span>
      )}
    </span>
  );
}

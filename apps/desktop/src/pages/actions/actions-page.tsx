// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  type AiAction,
  aiActionDelete,
  type AiActionInputSource,
  aiActionList,
  type AiActionOutputMode,
  type AiActionPermissionScope,
  aiActionSave,
  type AiActionSaveInput,
  type AiProvider,
  aiProviderList,
  createSnippet,
} from '@typvia/shared';
import { confirmLabel, modeLabel, scopeLabel, sourceLabel, useActionRun } from '@typvia/ui/ai';
import { useTr, type Tr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { Action, OverflowMenu, type MenuEntry } from '../../workspace/kit';
import './actions.css';

const SOURCE_KEYS: AiActionInputSource[] = ['selection', 'clipboard', 'snippet', 'share'];
const MODE_KEYS: AiActionOutputMode[] = ['replace', 'insert', 'copy', 'new_snippet'];
const SCOPE_KEYS: AiActionPermissionScope[] = ['normal_only', 'mask_secrets'];
/** Quiet, useful temperatures; the blank one means "the provider decides". */
const TEMPERATURES = ['', '0.2', '0.5', '0.7', '1.0'];
const AUTOSAVE_MS = 700;

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

/** An editable copy of one action; `id === null` is an unsaved new one. */
interface ActionDraft {
  id: string | null;
  name: string;
  promptTemplate: string;
  providerId: string | null;
  model: string;
  inputSource: AiActionInputSource;
  outputMode: AiActionOutputMode;
  permissionScope: AiActionPermissionScope;
  /** Kept as entered; blank means the provider's default. */
  temperature: string;
  isBuiltin: boolean;
}

function toDraft(action: AiAction): ActionDraft {
  return {
    id: action.id,
    name: action.name,
    promptTemplate: action.promptTemplate,
    providerId: action.providerId,
    model: action.model ?? '',
    inputSource: action.inputSource,
    outputMode: action.outputMode,
    permissionScope: action.permissionScope,
    temperature: action.temperature === null ? '' : String(action.temperature),
    isBuiltin: action.isBuiltin,
  };
}

function saveInput(draft: ActionDraft): AiActionSaveInput {
  const temperature = draft.temperature.trim();
  return {
    id: draft.id,
    name: draft.name.trim(),
    promptTemplate: draft.promptTemplate,
    providerId: draft.providerId,
    model: draft.model.trim() === '' ? null : draft.model.trim(),
    inputSource: draft.inputSource,
    outputMode: draft.outputMode,
    permissionScope: draft.permissionScope,
    temperature: temperature === '' ? null : Number(temperature),
  };
}

type Load =
  | { at: 'loading' }
  | { at: 'error' }
  | { at: 'off' }
  | { at: 'ready'; actions: AiAction[]; providers: AiProvider[] };

/**
 * AI Actions — an action studio, not a configuration
 * editor. Each action states its flow in one sentence of clickable tokens,
 * its instruction is the page's main surface, everything technical waits in
 * Advanced, and the test bench rises from the bottom only when asked for.
 */
export function ActionsPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const location = useLocation();
  const [load, setLoad] = useState<Load>({ at: 'loading' });
  const [draft, setDraft] = useState<ActionDraft | null>(null);
  const [composing, setComposing] = useState(false);
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const dirty = useRef(false);

  const load1 = useCallback(async (selectId?: string) => {
    try {
      const [actions, providers] = await Promise.all([aiActionList(), aiProviderList()]);
      if (providers.length === 0) {
        setLoad({ at: 'off' });
        setDraft(null);
        return;
      }
      setLoad({ at: 'ready', actions, providers });
      const selected =
        selectId === undefined ? actions[0] : actions.find((action) => action.id === selectId);
      setDraft(selected === undefined ? null : toDraft(selected));
    } catch {
      setLoad({ at: 'error' });
    }
  }, []);

  useEffect(() => {
    void load1();
  }, [load1]);

  // Quick create ("New AI action") arrives from Home with an intent in state.
  useEffect(() => {
    if (
      typeof location.state === 'object' &&
      location.state !== null &&
      'create' in location.state
    ) {
      setComposing(true);
    }
  }, [location.state]);

  /** Refreshes the list without disturbing the composer. */
  const refreshList = useCallback(async () => {
    try {
      const actions = await aiActionList();
      setLoad((current) => (current.at === 'ready' ? { ...current, actions } : current));
    } catch {
      // The list stays as it was; the composer is the source of truth here.
    }
  }, []);

  const edit = (next: ActionDraft) => {
    dirty.current = true;
    setDraft(next);
  };

  // Auto-save: a desktop tool saves itself. A draft without a name is
  // not yet an action, so it waits.
  useEffect(() => {
    if (!dirty.current || draft === null || draft.name.trim() === '') return;
    const timer = setTimeout(() => {
      dirty.current = false;
      setSaveState('saving');
      aiActionSave(saveInput(draft))
        .then(async (saved) => {
          setSaveState('saved');
          setDraft((current) =>
            current === null || current.id !== null ? current : { ...current, id: saved.id },
          );
          await refreshList();
        })
        .catch(() => setSaveState('error'));
    }, AUTOSAVE_MS);
    return () => clearTimeout(timer);
  }, [draft, refreshList]);

  useEffect(() => {
    if (saveState !== 'saved') return;
    const timer = setTimeout(() => setSaveState('idle'), 1600);
    return () => clearTimeout(timer);
  }, [saveState]);

  if (load.at === 'loading') {
    return (
      <div className="tva-single">
        <h1 style={{ fontSize: 26, fontWeight: 600, margin: 0 }}>{tr('AI Actions', 'AI 动作')}</h1>
      </div>
    );
  }

  if (load.at === 'error') {
    return (
      <div className="tva-single">
        <div className="tvw-empty">
          <strong>{tr('Your actions are safe.', '你的动作都还在。')}</strong>
          {tr('The list just could not load.', '只是列表暂时无法加载。')}
          <div style={{ marginTop: 10 }}>
            <button type="button" className="tvw-chip" onClick={() => void load1()}>
              {tr('Try again', '重试')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (load.at === 'off') {
    return (
      <div className="tva-single">
        <h1 style={{ fontSize: 26, fontWeight: 600, margin: 0 }}>{tr('AI Actions', 'AI 动作')}</h1>
        <div className="tvw-empty">
          <strong>{tr('Everything else works without AI.', '没有 AI 也不影响其他功能。')}</strong>
          {tr(
            'Actions need one provider — your own key, or a model on this Mac.',
            'AI 动作需要先配置一个 Provider——你自己的 Key,或本机的模型。',
          )}
          <div style={{ marginTop: 10 }}>
            <button type="button" className="tvw-chip" onClick={() => void navigate('/settings')}>
              {tr('Set up in Settings', '前往设置配置')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  const { actions, providers } = load;

  const duplicate = (action: AiAction) => {
    void aiActionSave({
      ...saveInput(toDraft(action)),
      id: null,
      name: tr(`${action.name} copy`, `${action.name} 副本`),
    }).then((saved) => load1(saved.id));
  };

  return (
    <div className="tva">
      <aside className="tva-list" aria-label={tr('Actions', '动作')}>
        <div className="tva-list-head">
          <span className="tvw-label">{tr('Actions', '动作')}</span>
          <Action
            label="＋"
            onRun={() => {
              setComposing(true);
              setDraft(null);
            }}
          />
        </div>
        {actions.length === 0 && (
          <p className="tva-note" style={{ padding: '4px 10px' }}>
            {tr('No actions yet.', '还没有动作。')}
          </p>
        )}
        {actions.map((action) => (
          <button
            key={action.id}
            type="button"
            className="tva-item"
            aria-current={!composing && action.id === draft?.id}
            onClick={() => {
              setComposing(false);
              dirty.current = false;
              setDraft(toDraft(action));
            }}
          >
            <span className="tva-item-row">
              <span className="tva-item-name">{action.name}</span>
              <span className="tvw-hover-actions">
                <OverflowMenu
                  label={tr('Action options', '动作选项')}
                  items={[
                    { label: tr('Duplicate', '创建副本'), onSelect: () => duplicate(action) },
                    'divider',
                    {
                      label: tr('Delete', '删除'),
                      danger: true,
                      onSelect: () => {
                        void aiActionDelete(action.id).then(() => load1());
                      },
                    },
                  ]}
                />
              </span>
            </span>
            <span className="tva-item-desc">{firstLine(action.promptTemplate)}</span>
          </button>
        ))}
      </aside>

      <section className="tva-composer" aria-label={tr('Action composer', '动作编辑')}>
        {composing || draft === null ? (
          <NewAction
            onCreate={(name, prompt) => {
              setComposing(false);
              dirty.current = true;
              setDraft({
                id: null,
                name,
                promptTemplate: prompt,
                providerId: providers[0]?.id ?? null,
                model: '',
                inputSource: 'selection',
                outputMode: 'copy',
                permissionScope: 'normal_only',
                temperature: '',
                isBuiltin: false,
              });
            }}
          />
        ) : (
          <Composer
            draft={draft}
            providers={providers}
            saveState={saveState}
            onEdit={edit}
            onDeleted={() => void load1()}
          />
        )}
      </section>
    </div>
  );
}

function firstLine(prompt: string): string {
  const line = prompt.split('\n').find((row) => row.trim() !== '') ?? '';
  return line.length > 64 ? `${line.slice(0, 64)}…` : line;
}

/** Ask what the action should do — never open an empty form. */
function NewAction({ onCreate }: { onCreate: (name: string, prompt: string) => void }) {
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
        <Action
          label={tr('Create', '创建')}
          cta
          disabled={text.trim() === ''}
          onRun={() => onCreate(text.trim(), '')}
        />
      </div>
      <div className="tva-templates">
        <span className="tvw-label" style={{ alignSelf: 'center', marginRight: 4 }}>
          {tr('Templates', '模板')}
        </span>
        {templates.map((template) => (
          <button
            key={template.label}
            type="button"
            className="tvw-chip"
            onClick={() => onCreate(template.label, template.prompt)}
          >
            {template.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function Composer({
  draft,
  providers,
  saveState,
  onEdit,
  onDeleted,
}: {
  draft: ActionDraft;
  providers: AiProvider[];
  saveState: 'idle' | 'saving' | 'saved' | 'error';
  onEdit: (next: ActionDraft) => void;
  onDeleted: () => void;
}) {
  const tr = useTr();
  const navigate = useNavigate();
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

  const menu: MenuEntry[] = [
    { label: tr('Rename', '重命名'), onSelect: () => setRenaming(true) },
    'divider',
    {
      label: tr('Delete', '删除'),
      danger: true,
      onSelect: () => {
        if (draft.id !== null) void aiActionDelete(draft.id).then(onDeleted);
      },
    },
  ];

  return (
    <>
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
              ? tr('Not saved — try again', '未保存——请重试')
              : tr('Saved', '已保存')}
        </span>
        <OverflowMenu label={tr('Action options', '动作选项')} items={menu} />
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
        <div className={`tvw-fold ${privacyOpen ? 'open' : ''}`}>
          <div>
            <div className="tvw-fold-inner tva-advanced">
              <div>
                <div className="tvw-label" style={{ marginBottom: 8 }}>
                  {tr('Sensitive content', '敏感内容')}
                </div>
                <div className="tva-choice-row">
                  {SCOPE_KEYS.map((key) => (
                    <button
                      key={key}
                      type="button"
                      className="tvw-chip"
                      aria-pressed={draft.permissionScope === key}
                      onClick={() => onEdit({ ...draft, permissionScope: key })}
                    >
                      {scopeLabel(key, tr)}
                    </button>
                  ))}
                </div>
                <p className="tva-note" style={{ marginTop: 8 }}>
                  {tr(
                    'Vault contents are never sent, whichever you pick.',
                    '无论选哪一项,保险库内容永远不会被发送。',
                  )}
                </p>
              </div>
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
        <div className={`tvw-fold ${advancedOpen ? 'open' : ''}`}>
          <div>
            <div className="tvw-fold-inner tva-advanced">
              <div>
                <div className="tvw-label" style={{ marginBottom: 8 }}>
                  {tr('Temperature', '温度')}
                </div>
                <div className="tva-choice-row">
                  {TEMPERATURES.map((value) => (
                    <button
                      key={value === '' ? 'default' : value}
                      type="button"
                      className="tvw-chip"
                      aria-pressed={draft.temperature === value}
                      onClick={() => onEdit({ ...draft, temperature: value })}
                    >
                      {value === '' ? tr('Provider default', 'Provider 默认') : value}
                    </button>
                  ))}
                </div>
              </div>
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

      {/* Test bench: folded away until someone wants to try it. */}
      <div className="tva-bench">
        <button
          type="button"
          className="tva-bench-toggle"
          aria-expanded={benchOpen}
          onClick={() => setBenchOpen((open) => !open)}
        >
          <span aria-hidden="true">{benchOpen ? '▾' : '▸'}</span>
          {tr('Test action', '试运行')}
        </button>
        {benchOpen && (
          <div className="tva-bench-body">
            <div className="tvw-label">{tr('Input', '输入')}</div>
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
              {draft.id === null && (
                <span className="tva-note">
                  {tr(
                    'Name the action first — it saves itself.',
                    '先给动作起个名字,它会自动保存。',
                  )}
                </span>
              )}
              <Action
                label={tr('Run  ⌘↵', '运行  ⌘↵')}
                cta
                disabled={draft.id === null || testText.trim() === '' || run.at === 'running'}
                onRun={() => void runAction()}
              />
            </div>

            {run.at === 'error' && (
              <div className="tva-inline-error" role="status">
                <strong>{tr('The provider did not answer', 'Provider 没有响应')}</strong>
                {tr(`Your text is unchanged — ${run.message}`, `你的文本没有改动——${run.message}`)}
                <div className="tva-inline-error-actions">
                  <Action label={tr('Retry', '重试')} cta onRun={() => void runAction()} />
                  <Action
                    label={tr('Provider settings', 'Provider 设置')}
                    onRun={() => void navigate('/settings')}
                  />
                </div>
              </div>
            )}

            {run.at === 'pending' && (
              <>
                <div className="tvw-label" style={{ marginTop: 16 }}>
                  {tr('Output', '输出')}
                </div>
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
                  <span className="tva-note">
                    {tr(
                      `${String(run.words)} words left this Mac · 1 request · ${run.seconds}s`,
                      `${String(run.words)} 个词离开了本机 · 1 次请求 · ${run.seconds} 秒`,
                    )}
                  </span>
                  <Action label={tr('Run again', '再次运行')} onRun={() => void runAction()} />
                  <Action label={tr('Dismiss', '关闭')} onRun={dismiss} />
                  <Action
                    label={`↵ ${confirmLabel(draft.outputMode, tr)}`}
                    cta
                    onRun={() => void confirm(run.output)}
                  />
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
    </>
  );
}

/** A clickable token in the flow sentence, with its choices in a popover. */
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
      if (wrap.current?.contains(event.target as Node) !== true) setOpen(false);
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
    <span ref={wrap} className="tvw-menu-wrap">
      <button
        type="button"
        className={`tva-token ${running ? 'is-running' : ''}`}
        aria-label={`${title}: ${label}`}
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        {label}
      </button>
      {open && (
        <span className="tvw-pop tva-pop" role="menu" aria-label={title}>
          <span className="tvw-pop-label">{title}</span>
          {children(() => setOpen(false))}
        </span>
      )}
    </span>
  );
}

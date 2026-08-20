import {
  createSnippet,
  historyList,
  ipcErrorCopy,
  listFolderChildren,
  updateSnippet,
  vaultCreateSecret,
} from '@typvia/shared';
import type { Folder, Snippet, SnippetHistory } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { previewIsMono } from '../../components/format';
import { pushBackHandler } from '../../platform';
import { AiStrip } from './ai-strip';
import './editor.css';

const TITLE_MAX = 60;
const HISTORY_PAGE = 20;

/**
 * Title fallback derived from the first content line at submit time — a
 * presentation default, not a business rule. Mirrors the desktop onboarding
 * derivation (first-snippet-step.tsx); the helper is local there too.
 */
function titleFromBody(body: string): string {
  const first = body.split('\n', 1)[0]?.trim() ?? '';
  return first.length > TITLE_MAX ? `${first.slice(0, TITLE_MAX - 1)}…` : first;
}

/** Type tabs as drawn plus Command — a first-class type the rest of the
 *  design draws in rows, a deliberate deviation. `sensitive` saves through
 *  the vault. */
const TYPE_CHOICES: ReadonlyArray<{ key: string; en: string; zh: string }> = [
  { key: 'text', en: 'Text', zh: '文本' },
  { key: 'code', en: 'Code', zh: '代码' },
  { key: 'command', en: 'Command', zh: '命令' },
  { key: 'prompt', en: 'Prompt', zh: '提示词' },
  { key: 'template', en: 'Template', zh: '模板' },
  { key: 'sensitive', en: 'Secret', zh: '密钥' },
];

/** Absolute version time — mobile shows the list without the diff. */
function versionTime(createdAt: number, locale: 'en' | 'zh'): string {
  return new Date(createdAt).toLocaleDateString(locale === 'zh' ? 'zh-CN' : 'en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

interface SnippetEditorPageProps {
  /** Present for edit; absent for create. Never a sensitive snippet. */
  snippet?: Snippet | undefined;
  onCancel: () => void;
  /** Called with the saved snippet; the host closes the sheet and toasts. */
  onSaved: (saved: Snippet) => void;
}

/**
 * Create / edit sheet: a 26px-radius
 * bottom sheet over a dimmed shell. Content first — the quick state is one
 * caret and a body field; anything typed expands the sheet into the tinted
 * content block with type tabs and the title/trigger/folder fields. The
 * sheet does no business validation: it submits and renders the core's
 * error verbatim, with the draft kept intact. Choosing Secret saves through
 * the vault (encrypt-before-store); a locked vault refuses honestly.
 */
export function SnippetEditorPage({ snippet, onCancel, onSaved }: SnippetEditorPageProps) {
  const tr = useTr();
  const locale = useLocale();
  const editing = snippet !== undefined;
  const [body, setBody] = useState(snippet?.body ?? '');
  const [title, setTitle] = useState(snippet?.title ?? '');
  const [snippetType, setSnippetType] = useState(snippet?.snippetType ?? 'text');
  const [folderId, setFolderId] = useState<string | null>(snippet?.folderId ?? null);
  const [trigger, setTrigger] = useState(snippet?.trigger ?? '');
  // null = folder roots still loading; a load failure reads as unavailable.
  const [folders, setFolders] = useState<Folder[] | null>(null);
  const [foldersFailed, setFoldersFailed] = useState(false);
  const [history, setHistory] = useState<SnippetHistory | null>(null);
  const [historyFailed, setHistoryFailed] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  // Height the soft keyboard takes from the visual viewport (see below).
  const [keyboardInset, setKeyboardInset] = useState(0);
  const saveRef = useRef<() => void>(() => {});

  const expanded = editing || body.trim() !== '';

  const typeChoices = TYPE_CHOICES.some((choice) => choice.key === snippetType)
    ? TYPE_CHOICES
    : [...TYPE_CHOICES, { key: snippetType, en: snippetType, zh: snippetType }];

  useEffect(() => {
    let cancelled = false;
    listFolderChildren(null)
      .then((roots) => {
        if (!cancelled) setFolders(roots);
      })
      .catch(() => {
        if (!cancelled) setFoldersFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const snippetId = snippet?.id;
  useEffect(() => {
    if (snippetId === undefined) return;
    let cancelled = false;
    historyList(snippetId, HISTORY_PAGE, 0)
      .then((loaded) => {
        if (!cancelled) setHistory(loaded);
      })
      .catch(() => {
        if (!cancelled) setHistoryFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [snippetId]);

  /*
   * Soft-keyboard occlusion, CSS-first: iOS keeps the layout viewport under
   * the keyboard and only shrinks the visual viewport. Padding the sheet by
   * the lost height gives every field room to scroll above the keyboard.
   */
  useEffect(() => {
    const viewport = window.visualViewport;
    if (viewport === null || viewport === undefined) return;
    const update = () => {
      setKeyboardInset(Math.max(0, window.innerHeight - viewport.height));
    };
    viewport.addEventListener('resize', update);
    return () => {
      viewport.removeEventListener('resize', update);
    };
  }, []);

  const canSave = body.trim() !== '' && !saving;

  const save = () => {
    if (!canSave) return;
    setSaving(true);
    setSaveError(null);
    const finalTitle = title.trim() === '' ? titleFromBody(body) : title.trim();
    const finalTrigger = trigger.trim() === '' ? null : trigger.trim();
    // Core requires trigger and trigger_mode to be set together; the sheet
    // has no mode field, so a trigger implies the immediate mode (an
    // existing mode on the snippet is preserved).
    const finalTriggerMode = finalTrigger === null ? null : (snippet?.triggerMode ?? 'immediate');
    const request = editing
      ? updateSnippet({
          id: snippet.id,
          title: finalTitle,
          body,
          snippetType,
          description: snippet.description,
          folderId,
          trigger: finalTrigger,
          triggerMode: finalTriggerMode,
          language: snippet.language,
          isFavorite: snippet.isFavorite,
          isPinned: snippet.isPinned,
          isEnabled: snippet.isEnabled,
        })
      : snippetType === 'sensitive'
        ? vaultCreateSecret({
            title: finalTitle,
            body,
            snippetType: 'sensitive',
            folderId,
          })
        : createSnippet({
            title: finalTitle,
            body,
            snippetType,
            folderId,
            trigger: finalTrigger,
            triggerMode: finalTriggerMode,
          });
    request.then(onSaved).catch((caught: unknown) => {
      // The wire message is rendered verbatim (core-owned copy).
      setSaveError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('it could not be written', '内容未能写入'),
      );
      setSaving(false);
    });
  };
  saveRef.current = save;

  // Android system back mirrors Escape while the sheet is open: the gesture
  // runs the same cancel path as the scrim tap (never backgrounds the app
  // under an open sheet). Inert off Android.
  useEffect(() => pushBackHandler(onCancel), [onCancel]);

  // Cmd/Ctrl+Enter saves, Escape cancels — full keyboard operability.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
        saveRef.current();
      } else if (event.key === 'Escape') {
        onCancel();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [onCancel]);

  return (
    <div
      className="tv-msheet-scrim"
      onPointerDown={(event) => {
        // Only a direct scrim tap dismisses; taps inside the sheet bubble.
        if (event.target === event.currentTarget) onCancel();
      }}
    >
      <section
        className="tv-msheet tv-med"
        role="dialog"
        aria-label={editing ? tr('Edit snippet', '编辑片段') : tr('New snippet', '新建片段')}
        style={keyboardInset > 0 ? { paddingBottom: keyboardInset } : undefined}
      >
        <div className="tv-msheet-handle" aria-hidden="true" />

        <header className="tv-med-head">
          <h1 className="tv-med-title">
            {editing ? tr('Edit snippet.', '编辑片段。') : tr('Save something.', '存点什么。')}
          </h1>
          {/* Drawn only on the expanded sheet (06); kept in the quick state
              too as the visible dismiss — onboarding uses it as its skip. */}
          <button type="button" className="tv-med-cancel" onClick={onCancel}>
            {tr('Cancel', '取消')}
          </button>
        </header>

        <textarea
          className={expanded && previewIsMono(snippetType) ? 'tv-med-body is-mono' : 'tv-med-body'}
          aria-label={tr('Snippet content', '片段内容')}
          placeholder={tr('Paste or start typing…', '粘贴或开始输入…')}
          rows={expanded ? 4 : 2}
          // Content first: the sheet opens straight into typing on create.
          autoFocus={!editing}
          value={body}
          onChange={(event) => {
            setBody(event.target.value);
          }}
        />

        {expanded && (
          <>
            <div className="tv-med-type-row">
              <span className="tv-med-caps">{tr('Type', '类型')}</span>
              <div
                role="group"
                aria-label={tr('Snippet type', '片段类型')}
                className="tv-med-type-tabs"
              >
                {typeChoices.map((choice) => {
                  const active = choice.key === snippetType;
                  return (
                    <button
                      key={choice.key}
                      type="button"
                      aria-pressed={active}
                      // The saved kind is fixed: converting an existing
                      // snippet to/from Secret is a vault flow, not a tab.
                      aria-disabled={(editing && choice.key === 'sensitive') || undefined}
                      className={active ? 'tv-med-type-tab is-active' : 'tv-med-type-tab'}
                      onClick={() => {
                        if (!(editing && choice.key === 'sensitive')) setSnippetType(choice.key);
                      }}
                    >
                      <span>{tr(choice.en, choice.zh)}</span>
                      {active && <span className="tv-med-type-bar" />}
                    </button>
                  );
                })}
              </div>
            </div>

            <label className="tv-med-field">
              <span className="tv-med-caps">{tr('Title', '标题')}</span>
              <input
                className="tv-med-input"
                type="text"
                placeholder={tr('From the first line', '默认取自第一行')}
                value={title}
                onChange={(event) => {
                  setTitle(event.target.value);
                }}
              />
            </label>

            {snippetType !== 'sensitive' && (
              <label className="tv-med-field">
                <span className="tv-med-caps">{tr('Trigger', '触发词')}</span>
                <input
                  className="tv-med-input is-mono"
                  type="text"
                  placeholder=";trigger"
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  value={trigger}
                  onChange={(event) => {
                    setTrigger(event.target.value);
                  }}
                />
              </label>
            )}

            <div className="tv-med-field tv-med-folder">
              <span className="tv-med-caps">{tr('Folder', '文件夹')}</span>
              {folders !== null && folders.length > 0 ? (
                <span className="tv-med-folder-control">
                  {/* aria-label (not a wrapping label) so the option texts
                      never leak into the control's accessible name. */}
                  <select
                    className="tv-med-select"
                    aria-label={tr('Folder', '文件夹')}
                    value={folderId ?? ''}
                    onChange={(event) => {
                      setFolderId(event.target.value === '' ? null : event.target.value);
                    }}
                  >
                    <option value="">{tr('None', '无')}</option>
                    {folders.map((folder) => (
                      <option key={folder.id} value={folder.id}>
                        {folder.name}
                      </option>
                    ))}
                  </select>
                  <span className="tv-med-chevron" aria-hidden="true">
                    ›
                  </span>
                </span>
              ) : (
                <span className="tv-med-field-meta">
                  {foldersFailed
                    ? tr('Folders unavailable right now', '文件夹暂时不可用')
                    : folders === null
                      ? ''
                      : tr('No folders yet', '还没有文件夹')}
                </span>
              )}
            </div>

            {editing && (
              <button
                type="button"
                className="tv-med-more"
                aria-expanded={moreOpen}
                onClick={() => {
                  setMoreOpen((open) => !open);
                }}
              >
                {tr('More options', '更多选项')}
                <span aria-hidden="true" className="tv-med-more-glyph">
                  {moreOpen ? '⌃' : '⌄'}
                </span>
              </button>
            )}

            {editing && moreOpen && (
              <section className="tv-med-history" aria-label={tr('Version history', '版本历史')}>
                <div className="tv-med-caps">{tr('History', '版本')}</div>
                {historyFailed ? (
                  <p className="tv-med-history-meta">
                    {tr(
                      'The versions could not be loaded — the snippet itself is unaffected.',
                      '版本列表未能加载——片段本身不受影响。',
                    )}
                  </p>
                ) : history === null ? (
                  <p className="tv-med-history-meta">{tr('Loading versions…', '正在加载版本…')}</p>
                ) : history.entries.length === 0 ? (
                  <p className="tv-med-history-meta">
                    {tr('No earlier versions yet.', '还没有更早的版本。')}
                  </p>
                ) : (
                  <ul className="tv-med-history-rows">
                    {history.entries.map((entry) => (
                      <li key={entry.version} className="tv-med-history-row">
                        <span className="tv-med-history-version">v{entry.version}</span>
                        <span className="tv-med-history-time">
                          {versionTime(entry.createdAt, locale)}
                          {entry.version === history.current ? tr(' · current', ' · 当前') : ''}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            )}

            {snippetType !== 'sensitive' && (
              <AiStrip
                snippetId={snippet?.id ?? null}
                title={title}
                body={body}
                snippetType={snippetType}
                folderId={folderId}
                trigger={trigger}
                description={snippet?.description ?? null}
                onApply={(changes) => {
                  if (changes.title !== undefined) setTitle(changes.title);
                  if (changes.snippetType !== undefined) setSnippetType(changes.snippetType);
                  if (changes.trigger !== undefined) setTrigger(changes.trigger);
                  if (changes.folderId !== undefined) setFolderId(changes.folderId);
                }}
                onBodyChange={setBody}
              />
            )}
          </>
        )}

        {saveError !== null && (
          <p className="tv-med-error" role="alert">
            {tr(
              'Nothing was saved — your draft is intact.',
              '没有保存任何内容——你的草稿完好无损。',
            )}
            <span className="tv-med-error-detail">{saveError}</span>
          </p>
        )}

        <div className="tv-med-bar">
          <button type="button" className="tv-med-save" disabled={!canSave} onClick={save}>
            {saving ? tr('Saving…', '保存中…') : tr('Save', '保存')}
          </button>
        </div>
      </section>
    </div>
  );
}

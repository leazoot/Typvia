import { StatusDot } from '@typvia/ui';
import { useState } from 'react';
import type { FolderEntry } from '../library/rail';
import { variableNames } from './editor-body';
import type { Draft } from './use-editor-draft';

type TabKey = 'fields' | 'usage' | 'security' | 'advanced';

const TABS: Array<{ key: TabKey; label: string }> = [
  { key: 'fields', label: 'Fields' },
  { key: 'usage', label: 'Where it can be used' },
  { key: 'security', label: 'Security' },
  { key: 'advanced', label: 'Advanced' },
];

const TRIGGER_MODES: Array<{ value: string; label: string }> = [
  { value: 'delimiter', label: 'After delimiter' },
  { value: 'immediate', label: 'Immediately' },
  { value: 'word_boundary', label: 'On word boundary' },
  { value: 'regex', label: 'Regular expression' },
];

/** Editable types in v1 — `sensitive` is the vault flow, `temporary` is OQ-R4. */
const TYPE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'text', label: 'Text' },
  { value: 'markdown', label: 'Markdown' },
  { value: 'code', label: 'Code' },
  { value: 'command', label: 'Command' },
  { value: 'prompt', label: 'Prompt' },
  { value: 'template', label: 'Template' },
  { value: 'ai_action', label: 'AI action' },
  { value: 'link', label: 'Link' },
];

/** Human words for detection codes (colour never carries this alone). */
const KIND_WORDS: Record<string, string> = {
  pem_private_key: 'private key',
  jwt: 'JWT',
  bearer_token: 'bearer token',
  aws_access_key: 'AWS key',
  github_token: 'GitHub token',
  api_key: 'API key',
  db_connection_string: 'database credentials',
  cookie: 'cookie',
  high_entropy_string: 'high-entropy string',
  password_field: 'password',
};

interface EditorTabsProps {
  draft: Draft;
  onPatch: (changes: Partial<Draft>) => void;
  folders: FolderEntry[];
  sensitiveKinds: string[];
  /** Business error from the last save (e.g. trigger conflict). */
  errorMessage: string | null;
}

/**
 * The bottom property band (design 1c): four tabs, and only the open tab's
 * fields exist in the DOM — low-frequency settings cost nothing.
 */
export function EditorTabs({
  draft,
  onPatch,
  folders,
  sensitiveKinds,
  errorMessage,
}: EditorTabsProps) {
  const [open, setOpen] = useState<TabKey>('fields');
  const variables = variableNames(draft.body);

  return (
    <div className="tv-ed-band">
      <div className="tv-ed-band-tabs" role="tablist">
        {TABS.map((tab) => (
          <button
            key={tab.key}
            type="button"
            role="tab"
            aria-selected={open === tab.key}
            className="tv-ed-band-tab"
            data-active={open === tab.key ? true : undefined}
            onClick={() => {
              setOpen(tab.key);
            }}
          >
            {tab.label}
            {open === tab.key && <span aria-hidden="true" className="tv-ed-band-caret" />}
          </button>
        ))}
      </div>
      <div className="tv-ed-band-panel" role="tabpanel">
        {open === 'fields' && (
          <div>
            {variables.length === 0 ? (
              <p className="tv-ed-band-note">
                No variables in this body — type <code>{'{{name}}'}</code> to mark a fill-in blank.
              </p>
            ) : (
              <div className="tv-ed-band-vars">
                {variables.map((name) => (
                  <span key={name} className="tv-ed-token">{`{{${name}}}`}</span>
                ))}
              </div>
            )}
            <p className="tv-ed-band-note tv-ed-band-note-cn" lang="zh-Hans">
              字段配置(类型/默认值/必填)随模板编辑器批次到来。
            </p>
          </div>
        )}
        {open === 'usage' && (
          <div className="tv-ed-band-grid">
            <label className="tv-ed-field">
              <span>Trigger</span>
              <input
                type="text"
                className="tv-ed-input tv-ed-input-mono"
                placeholder=";example"
                value={draft.trigger ?? ''}
                onChange={(event) => {
                  const value = event.target.value;
                  onPatch({ trigger: value === '' ? null : value });
                }}
              />
            </label>
            <label className="tv-ed-field">
              <span>Expands</span>
              <select
                className="tv-ed-input"
                value={draft.triggerMode ?? 'delimiter'}
                disabled={draft.trigger === null}
                onChange={(event) => {
                  onPatch({ triggerMode: event.target.value });
                }}
              >
                {TRIGGER_MODES.map((mode) => (
                  <option key={mode.value} value={mode.value}>
                    {mode.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="tv-ed-field tv-ed-field-check">
              <input
                type="checkbox"
                checked={draft.isEnabled}
                onChange={(event) => {
                  onPatch({ isEnabled: event.target.checked });
                }}
              />
              <span>Enabled</span>
            </label>
            {errorMessage !== null && (
              <p className="tv-ed-band-error">
                <StatusDot kind="error" label={errorMessage} />
              </p>
            )}
          </div>
        )}
        {open === 'security' && (
          <div>
            <StatusDot kind="secure" label="Not a secret — this snippet may be sent to AI." />
            <p className="tv-ed-band-note tv-ed-band-note-cn" lang="zh-Hans">
              改为 Secret 后将默认遮挡并要求验证(保险库随后续批次到来)。
            </p>
            {sensitiveKinds.length > 0 && (
              <p className="tv-ed-band-hint">
                Looks like this may contain:{' '}
                {sensitiveKinds.map((kind) => KIND_WORDS[kind] ?? kind).join(' · ')}. Saving keeps
                it as normal text — this is only a suggestion.
              </p>
            )}
          </div>
        )}
        {open === 'advanced' && (
          <div className="tv-ed-band-grid">
            <label className="tv-ed-field">
              <span>Type</span>
              <select
                className="tv-ed-input"
                value={draft.snippetType}
                onChange={(event) => {
                  onPatch({ snippetType: event.target.value });
                }}
              >
                {TYPE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="tv-ed-field">
              <span>Folder</span>
              <select
                className="tv-ed-input"
                value={draft.folderId ?? ''}
                onChange={(event) => {
                  const value = event.target.value;
                  onPatch({ folderId: value === '' ? null : value });
                }}
              >
                <option value="">Unsorted</option>
                {folders.map(({ folder, depth }) => (
                  <option key={folder.id} value={folder.id}>
                    {`${' '.repeat(depth * 2)}${folder.name}`}
                  </option>
                ))}
              </select>
            </label>
            <label className="tv-ed-field">
              <span>Language</span>
              <input
                type="text"
                className="tv-ed-input tv-ed-input-mono"
                placeholder="e.g. sql"
                value={draft.language ?? ''}
                onChange={(event) => {
                  const value = event.target.value;
                  onPatch({ language: value === '' ? null : value });
                }}
              />
            </label>
            <label className="tv-ed-field tv-ed-field-check">
              <input
                type="checkbox"
                checked={draft.isFavorite}
                onChange={(event) => {
                  onPatch({ isFavorite: event.target.checked });
                }}
              />
              <span>Starred</span>
            </label>
            <label className="tv-ed-field tv-ed-field-check">
              <input
                type="checkbox"
                checked={draft.isPinned}
                onChange={(event) => {
                  onPatch({ isPinned: event.target.checked });
                }}
              />
              <span>Pinned</span>
            </label>
          </div>
        )}
      </div>
    </div>
  );
}

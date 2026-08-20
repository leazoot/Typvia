import { snippetConvertToSensitive } from '@typvia/shared';
import { StatusDot, useTr, type Tr } from '@typvia/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useVault } from '../../vault/vault-context';
import type { FolderEntry } from '../library/rail';
import { variableNames } from './editor-body';
import type { Draft } from './use-editor-draft';

type TabKey = 'fields' | 'usage' | 'security' | 'advanced';

const TABS: Array<{ key: TabKey; label: readonly [string, string] }> = [
  { key: 'fields', label: ['Fields', '字段'] },
  { key: 'usage', label: ['Where it can be used', '可用范围'] },
  { key: 'security', label: ['Security', '安全'] },
  { key: 'advanced', label: ['Advanced', '高级'] },
];

const TRIGGER_MODES: Array<{ value: string; label: readonly [string, string] }> = [
  { value: 'delimiter', label: ['After delimiter', '分隔符之后'] },
  { value: 'immediate', label: ['Immediately', '立即'] },
  { value: 'word_boundary', label: ['On word boundary', '词边界'] },
  { value: 'regex', label: ['Regular expression', '正则表达式'] },
];

/** Editable types in v1 — `sensitive` is the vault flow, `temporary` is undecided. */
const TYPE_OPTIONS: Array<{ value: string; label: readonly [string, string] }> = [
  { value: 'text', label: ['Text', '文本'] },
  { value: 'markdown', label: ['Markdown', 'Markdown'] },
  { value: 'code', label: ['Code', '代码'] },
  { value: 'command', label: ['Command', '命令'] },
  { value: 'prompt', label: ['Prompt', '提示词'] },
  { value: 'template', label: ['Template', '模板'] },
  { value: 'ai_action', label: ['AI action', 'AI 操作'] },
  { value: 'link', label: ['Link', '链接'] },
];

/** Human words for detection codes (colour never carries this alone). */
const KIND_WORDS: Record<string, readonly [string, string]> = {
  pem_private_key: ['private key', '私钥'],
  jwt: ['JWT', 'JWT'],
  bearer_token: ['bearer token', 'Bearer 令牌'],
  aws_access_key: ['AWS key', 'AWS 密钥'],
  github_token: ['GitHub token', 'GitHub 令牌'],
  api_key: ['API key', 'API 密钥'],
  db_connection_string: ['database credentials', '数据库凭据'],
  cookie: ['cookie', 'Cookie'],
  high_entropy_string: ['high-entropy string', '高熵字符串'],
  password_field: ['password', '密码'],
};

/** Resolves a detection code to one language, falling back to the raw code. */
function kindWord(kind: string, tr: Tr): string {
  const pair = KIND_WORDS[kind];
  return pair === undefined ? kind : tr(...pair);
}

interface EditorTabsProps {
  draft: Draft;
  onPatch: (changes: Partial<Draft>) => void;
  folders: FolderEntry[];
  sensitiveKinds: string[];
  /** Business error from the last save (e.g. trigger conflict). */
  errorMessage: string | null;
  /** True once the draft is persisted — moving to the vault needs a saved id. */
  saved: boolean;
}

/**
 * The Security tab's body. Turning a normal snippet into a vault Secret happens
 * here: it encrypts the current value, drops its plaintext from search and
 * history, and hands the snippet to the vault. The move needs a saved snippet
 * and an unlocked vault; both are surfaced inline (no modal — the only modal in
 * the product is the vault unlock). Confirmation is inline, and a failure says
 * what is still safe before asking the user to retry.
 */
function SecurityTab({
  draft,
  sensitiveKinds,
  saved,
}: {
  draft: Draft;
  sensitiveKinds: string[];
  saved: boolean;
}) {
  const tr = useTr();
  const navigate = useNavigate();
  const { status: vault } = useVault();
  const [confirming, setConfirming] = useState(false);
  const [working, setWorking] = useState(false);
  const [failed, setFailed] = useState(false);

  if (draft.snippetType === 'sensitive') {
    return (
      <div>
        <StatusDot
          kind="secure"
          label={tr(
            'A secret — masked by default and kept in the vault.',
            '这是 Secret——默认遮挡,存放于保险库。',
          )}
        />
        <p className="tv-ed-band-note tv-ed-band-note-sub">
          {tr('The body lives only in the vault — go to', '正文只存于保险库,前往')}{' '}
          <button type="button" className="tv-ed-band-link" onClick={() => void navigate('/vault')}>
            {tr('Vault', '保险库')}
          </button>{' '}
          {tr('to view or edit it.', '查看或编辑。')}
        </p>
      </div>
    );
  }

  const initialized = vault?.initialized === true;
  const unlocked = vault?.unlocked === true;

  async function convert() {
    if (draft.id === null) return;
    setWorking(true);
    setFailed(false);
    try {
      await snippetConvertToSensitive(draft.id);
      void navigate('/vault');
    } catch {
      setFailed(true);
      setWorking(false);
      setConfirming(false);
    }
  }

  return (
    <div>
      <StatusDot
        kind="secure"
        label={tr(
          'Not a secret — this snippet may be sent to AI.',
          '非 Secret——该片段可能会发送给 AI。',
        )}
      />
      <div className="tv-ed-convert">
        {!saved || draft.id === null ? (
          <p className="tv-ed-band-note">
            {tr('Save this snippet before moving it to the vault.', '先保存该片段,再移入保险库。')}
          </p>
        ) : !initialized ? (
          <p className="tv-ed-band-note">
            {tr('Set up the vault first —', '请先设置保险库——')}{' '}
            <button
              type="button"
              className="tv-ed-band-link"
              onClick={() => void navigate('/vault')}
            >
              {tr('open the vault', '打开保险库')}
            </button>
            {tr('.', '。')}
          </p>
        ) : !unlocked ? (
          <p className="tv-ed-band-note">
            {tr('Unlock the vault to move this in —', '解锁保险库后即可移入——')}{' '}
            <button
              type="button"
              className="tv-ed-band-link"
              onClick={() => void navigate('/vault')}
            >
              {tr('open the vault', '打开保险库')}
            </button>
            {tr('.', '。')}
          </p>
        ) : confirming ? (
          <div>
            <p className="tv-ed-band-note">
              {tr(
                'This encrypts the value and removes its plaintext from search and version history.',
                '此操作会加密正文,并从搜索与版本历史中移除明文。',
              )}
            </p>
            <div className="tv-ed-convert-actions">
              <button
                type="button"
                className="tv-ed-convert-go"
                disabled={working}
                onClick={() => void convert()}
              >
                {tr('Move to vault', '移入保险库')}
              </button>
              <button
                type="button"
                className="tv-ed-band-link"
                disabled={working}
                onClick={() => setConfirming(false)}
              >
                {tr('Cancel', '取消')}
              </button>
            </div>
          </div>
        ) : (
          <>
            <button type="button" className="tv-ed-convert-go" onClick={() => setConfirming(true)}>
              {tr('Move to the vault', '移入保险库')}
            </button>
            <p className="tv-ed-band-note tv-ed-band-note-sub">
              {tr(
                'Becoming a Secret encrypts the body and removes its plaintext from search and history.',
                '转为 Secret:加密正文,从搜索与历史移除明文。',
              )}
            </p>
          </>
        )}
      </div>
      {failed && (
        <p className="tv-ed-band-note">
          {tr(
            'Your snippet is unchanged and safe — the move didn’t go through. Try again.',
            '片段未受影响且安全——移动未完成,请重试。',
          )}
        </p>
      )}
      {sensitiveKinds.length > 0 && (
        <p className="tv-ed-band-hint">
          {tr(
            `Looks like this may contain: ${sensitiveKinds.map((kind) => kindWord(kind, tr)).join(' · ')}.`,
            `似乎包含:${sensitiveKinds.map((kind) => kindWord(kind, tr)).join(' · ')}。`,
          )}{' '}
          {initialized && unlocked
            ? tr('Moving it to the vault encrypts and masks it.', '移入保险库后将加密并遮挡。')
            : tr(
                'Saving keeps it as normal text — this is only a suggestion.',
                '保存后仍是普通文本——这只是一个提示。',
              )}
        </p>
      )}
    </div>
  );
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
  saved,
}: EditorTabsProps) {
  const tr = useTr();
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
            {tr(...tab.label)}
            {open === tab.key && <span aria-hidden="true" className="tv-ed-band-caret" />}
          </button>
        ))}
      </div>
      <div className="tv-ed-band-panel" role="tabpanel">
        {open === 'fields' && (
          <div>
            {variables.length === 0 ? (
              <p className="tv-ed-band-note">
                {tr('No variables in this body — type', '正文中没有变量——输入')}{' '}
                <code>{'{{name}}'}</code> {tr('to mark a fill-in blank.', '即可标记一个填空。')}
              </p>
            ) : (
              <div className="tv-ed-band-vars">
                {variables.map((name) => (
                  <span key={name} className="tv-ed-token">{`{{${name}}}`}</span>
                ))}
              </div>
            )}
            <p className="tv-ed-band-note tv-ed-band-note-sub">
              {tr(
                'Field settings (type, default value, required) arrive with the template editor.',
                '字段配置(类型/默认值/必填)随模板编辑器批次到来。',
              )}
            </p>
          </div>
        )}
        {open === 'usage' && (
          <div className="tv-ed-band-grid">
            <label className="tv-ed-field">
              <span>{tr('Trigger', '触发词')}</span>
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
              <span>{tr('Expands', '展开时机')}</span>
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
                    {tr(...mode.label)}
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
              <span>{tr('Enabled', '启用')}</span>
            </label>
            {errorMessage !== null && (
              <p className="tv-ed-band-error">
                <StatusDot kind="error" label={errorMessage} />
              </p>
            )}
          </div>
        )}
        {open === 'security' && (
          <SecurityTab draft={draft} sensitiveKinds={sensitiveKinds} saved={saved} />
        )}
        {open === 'advanced' && (
          <div className="tv-ed-band-grid">
            <label className="tv-ed-field">
              <span>{tr('Type', '类型')}</span>
              <select
                className="tv-ed-input"
                value={draft.snippetType}
                onChange={(event) => {
                  onPatch({ snippetType: event.target.value });
                }}
              >
                {TYPE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {tr(...option.label)}
                  </option>
                ))}
              </select>
            </label>
            <label className="tv-ed-field">
              <span>{tr('Folder', '文件夹')}</span>
              <select
                className="tv-ed-input"
                value={draft.folderId ?? ''}
                onChange={(event) => {
                  const value = event.target.value;
                  onPatch({ folderId: value === '' ? null : value });
                }}
              >
                <option value="">{tr('Unsorted', '未分类')}</option>
                {folders.map(({ folder, depth }) => (
                  <option key={folder.id} value={folder.id}>
                    {`${' '.repeat(depth * 2)}${folder.name}`}
                  </option>
                ))}
              </select>
            </label>
            <label className="tv-ed-field">
              <span>{tr('Language', '语言')}</span>
              <input
                type="text"
                className="tv-ed-input tv-ed-input-mono"
                placeholder={tr('e.g. sql', '例如 sql')}
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
              <span>{tr('Starred', '星标')}</span>
            </label>
            <label className="tv-ed-field tv-ed-field-check">
              <input
                type="checkbox"
                checked={draft.isPinned}
                onChange={(event) => {
                  onPatch({ isPinned: event.target.checked });
                }}
              />
              <span>{tr('Pinned', '置顶')}</span>
            </label>
          </div>
        )}
      </div>
    </div>
  );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Vault — "keep secrets usable without ever displaying
 * them", in a room that is quieter than the rest of the workspace rather than
 * a different product. Four states off the vault session status: loading,
 * first run, locked (in place — no modal), unlocked.
 *
 * Security is expressed by a square outline, never a filled circle or a red
 * warning; the auto-lock countdown is a hairline plus a clock, never a dial.
 */
import {
  IpcError,
  ipcErrorCopy,
  panelCopySecret,
  type Snippet,
  trashSnippet,
  vaultCreateSecret,
  vaultEnableBiometric,
  vaultInitialize,
  vaultList,
  vaultReveal,
  type VaultStatus,
  vaultUpdateSecret,
} from '@typvia/shared';
import { TypeMark, useTr } from '@typvia/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router';
import {
  Action,
  Dot,
  Glyph,
  OverflowMenu,
  Row,
  useContextMenu,
  type MenuEntry,
} from '../../workspace/kit';
import { useVault } from '../../vault/vault-context';
import { LockedPanel } from './locked-panel';
import './vault.css';

/** A revealed secret re-masks itself after this window. */
const REVEAL_TTL_MS = 8_000;
/** Remembers that the Touch ID offer was already made once. */
const TOUCH_ID_ASKED_KEY = 'tv.ui.vaultTouchIdAsked';

export function VaultPage() {
  const { status, apply, unlockBiometric, lock } = useVault();
  const tr = useTr();
  const location = useLocation();

  if (status === null) {
    return (
      <div className="tvv" aria-live="polite">
        <div className="tvv-head">
          <h1>{tr('Vault', '保险库')}</h1>
        </div>
      </div>
    );
  }

  if (!status.initialized) return <VaultFirstRun onInitialized={apply} />;

  if (!status.unlocked) {
    return <VaultLocked onUnlocked={apply} onUnlockBiometric={unlockBiometric} />;
  }

  return (
    <VaultUnlocked
      status={status}
      onLock={lock}
      startCreating={
        typeof location.state === 'object' && location.state !== null && 'create' in location.state
      }
    />
  );
}

// --- Locked ----------------------------------------------------------------

function VaultLocked({
  onUnlocked,
  onUnlockBiometric,
}: {
  onUnlocked: (status: VaultStatus) => void;
  onUnlockBiometric: () => Promise<void>;
}) {
  const tr = useTr();
  const [count, setCount] = useState(0);

  useEffect(() => {
    // Metadata only — nothing is decrypted to know how much is in there.
    vaultList(500, 0)
      .then((rows) => setCount(rows.length))
      .catch(() => setCount(0));
  }, []);

  return (
    <div className="tvv">
      <div className="tvv-head">
        <div>
          <h1>{tr('Vault', '保险库')}</h1>
          <div className="tvv-state">
            <Dot kind="idle" />
            {tr('Locked', '已锁定')}
          </div>
        </div>
      </div>
      <LockedPanel
        itemCount={count}
        onUnlocked={onUnlocked}
        onUnlockBiometric={onUnlockBiometric}
        onReset={onUnlocked}
      />
    </div>
  );
}

// --- First run -------------------------------------------------------------

function VaultFirstRun({ onInitialized }: { onInitialized: (status: VaultStatus) => void }) {
  const tr = useTr();
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (busy) return;
      if (password.length < 8) {
        setError(
          tr('Choose a master password of at least 8 characters.', '请设置至少 8 个字符的主密码。'),
        );
        return;
      }
      if (password !== confirm) {
        setError(tr('The two passwords do not match.', '两次输入的密码不一致。'));
        return;
      }
      setBusy(true);
      setError(null);
      try {
        onInitialized(await vaultInitialize(password));
      } catch (raw) {
        setError(
          raw instanceof IpcError
            ? tr(...ipcErrorCopy(raw))
            : tr('could not set up the vault', '未能设置保险库'),
        );
      } finally {
        setBusy(false);
        setPassword('');
        setConfirm('');
      }
    },
    [password, confirm, busy, onInitialized, tr],
  );

  return (
    <div className="tvv">
      <div className="tvv-head">
        <h1>{tr('Set up the vault', '设置保险库')}</h1>
      </div>
      <form className="tvv-form" onSubmit={submit} style={{ maxWidth: 460 }}>
        <label className="tvv-field">
          <span>{tr('Master password', '主密码')}</span>
          <input
            type="password"
            aria-label={tr('Master password', '主密码')}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            disabled={busy}
          />
        </label>
        <label className="tvv-field">
          <span>{tr('Confirm', '确认密码')}</span>
          <input
            type="password"
            aria-label={tr('Confirm master password', '确认主密码')}
            value={confirm}
            onChange={(event) => setConfirm(event.target.value)}
            disabled={busy}
          />
        </label>
        <div className="tvv-form-actions">
          <button type="submit" className="tvw-chip" disabled={busy}>
            {tr('Create vault', '创建保险库')}
          </button>
        </div>
      </form>
      <p className="tvv-error" role={error === null ? undefined : 'alert'}>
        {error ??
          tr(
            'There is no recovery if you forget it — Typvia never sees your master password.',
            '忘记主密码将无法找回——Typvia 永远不会看到你的主密码。',
          )}
      </p>
    </div>
  );
}

// --- Unlocked --------------------------------------------------------------

function VaultUnlocked({
  status,
  onLock,
  startCreating,
}: {
  status: VaultStatus;
  onLock: () => Promise<void>;
  startCreating: boolean;
}) {
  const tr = useTr();
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [query, setQuery] = useState('');
  const [creating, setCreating] = useState(startCreating);
  const [securityOpen, setSecurityOpen] = useState(false);
  const [timerOpen, setTimerOpen] = useState(false);
  const context = useContextMenu(tr('Secret actions', '密文操作'));

  const load = useCallback(async () => {
    try {
      setRows(await vaultList(500, 0));
      setFailed(false);
    } catch {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const filtered = useMemo(() => {
    if (rows === null) return null;
    const needle = query.trim().toLowerCase();
    if (needle === '') return rows;
    return rows.filter((row) => row.title.toLowerCase().includes(needle));
  }, [rows, query]);

  return (
    <div className="tvv">
      <div className="tvv-head">
        <div>
          <h1>{tr('Vault', '保险库')}</h1>
          <div className="tvv-state">
            <Dot kind="ok" />
            {tr('Unlocked', '已解锁')}
            <span className="tvw-menu-wrap">
              <RelockTimer
                status={status}
                expanded={timerOpen}
                onToggle={() => setTimerOpen((open) => !open)}
              />
              {timerOpen && (
                <span className="tvw-menu" role="menu">
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setTimerOpen(false);
                      void onLock();
                    }}
                  >
                    {tr('Lock now', '立即锁定')}
                  </button>
                </span>
              )}
            </span>
          </div>
          <div className="tvv-sub">
            {rows === null
              ? tr('Decrypted on this Mac only', '仅在本机解密')
              : tr(
                  `${String(rows.length)} secret${rows.length === 1 ? '' : 's'} · decrypted on this Mac only`,
                  `${String(rows.length)} 条密文 · 仅本机解密`,
                )}
          </div>
        </div>
        <div className="tvv-actions">
          <Action label={tr('Lock', '锁定')} onRun={() => void onLock()} />
          <button
            type="button"
            className="tvw-chip"
            aria-expanded={creating}
            onClick={() => setCreating((open) => !open)}
          >
            {tr('New secret', '新建密文')}
          </button>
        </div>
      </div>

      {creating && (
        <SecretForm
          submitLabel={tr('Save secret', '保存密文')}
          onCancel={() => setCreating(false)}
          onSave={async (title, body) => {
            await vaultCreateSecret({
              title,
              body,
              snippetType: 'sensitive',
              description: null,
              folderId: null,
              trigger: null,
              triggerMode: null,
              language: null,
            });
            setCreating(false);
            await load();
          }}
        />
      )}

      <div className="tvv-search">
        <Glyph name="search" />
        <input
          type="text"
          value={query}
          aria-label={tr('Search the vault by title', '按标题搜索保险库')}
          placeholder={tr('Search the vault', '搜索保险库')}
          onChange={(event) => setQuery(event.target.value)}
        />
        <span className="tvv-secure" tabIndex={0} role="note">
          <span aria-hidden="true" className="tvv-secure-mark" />
          <span className="tvv-secure-note">
            {tr(
              'Titles only — secret contents never enter the search index.',
              '仅搜索标题——密文内容从不进入搜索索引。',
            )}
          </span>
        </span>
      </div>

      {failed ? (
        <div className="tvw-empty">
          <strong>
            {tr('Your secrets are safe and encrypted on this Mac.', '你的密文仍安全地加密在本机。')}
          </strong>
          {tr('The list just failed to load.', '只是列表刚才未能加载。')}
        </div>
      ) : filtered === null ? null : filtered.length === 0 ? (
        <div className="tvw-empty">
          <strong>
            {query.trim() === ''
              ? tr('The vault is empty.', '保险库还是空的。')
              : tr('No secret with that title.', '没有同名的密文。')}
          </strong>
          {query.trim() === ''
            ? tr(
                'What you add here is encrypted before it is saved, and never indexed.',
                '在这里添加的内容会先加密再保存,并且从不进入索引。',
              )
            : tr(
                'Contents are never searched — only titles are.',
                '密文内容从不被搜索——只搜索标题。',
              )}
        </div>
      ) : (
        filtered.map((row) => (
          <SecretRow key={row.id} row={row} onChanged={load} context={context} />
        ))
      )}

      <TouchIdOffer />

      <section className="tvv-security" aria-label={tr('Security', '安全')}>
        <button
          type="button"
          className="tvv-security-row"
          aria-expanded={securityOpen}
          onClick={() => setSecurityOpen((open) => !open)}
        >
          <span className="tvv-security-line">
            <span className="tvw-label">{tr('Security', '安全')}</span>
            <span>{tr('Auto-lock · 5 min', '自动锁定 · 5 分钟')}</span>
            <span>{tr('Local encryption', '本机加密')}</span>
            <span>{tr('AI access · never', 'AI 访问 · 从不')}</span>
          </span>
          <span aria-hidden="true" className="tvv-security-chev">
            ›
          </span>
        </button>
        <div className={`tvw-fold ${securityOpen ? 'open' : ''}`}>
          <div>
            <div className="tvw-fold-inner tvv-policy">
              {[
                tr(
                  'The vault locks itself after 5 minutes without activity.',
                  '闲置 5 分钟后保险库自动锁定。',
                ),
                tr(
                  'Contents are encrypted on this Mac and never enter the search index.',
                  '内容在本机加密,从不进入搜索索引。',
                ),
                tr(
                  'AI actions can never read vault contents, and the sync server only ever holds ciphertext.',
                  'AI 动作永远无法读取保险库内容,同步服务器也只持有密文。',
                ),
                tr(
                  'A copied secret is cleared from the clipboard automatically.',
                  '复制的密文会自动从剪贴板清除。',
                ),
              ].map((line) => (
                <p key={line} className="tvv-policy-item">
                  <span aria-hidden="true" className="tvv-square" />
                  <span>{line}</span>
                </p>
              ))}
              <BiometricSetting />
            </div>
          </div>
        </div>
      </section>
      {context.node}
    </div>
  );
}

function RelockTimer({
  status,
  expanded,
  onToggle,
}: {
  status: VaultStatus;
  expanded: boolean;
  onToggle: () => void;
}) {
  const tr = useTr();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(timer);
  }, []);
  const base = status.lastActivityAt ?? status.unlockedAt;
  if (base === null) return null;
  const remaining = Math.max(0, base + status.idleTimeoutMs - now);
  const minutes = Math.floor(remaining / 60_000);
  const seconds = Math.floor((remaining % 60_000) / 1_000)
    .toString()
    .padStart(2, '0');
  const clock = `${String(minutes)}:${seconds}`;
  return (
    <button
      type="button"
      className="tvv-timer"
      aria-expanded={expanded}
      aria-label={tr(`Locks automatically in ${clock}`, `${clock} 后自动锁定`)}
      onClick={onToggle}
    >
      <span aria-hidden="true" className="tvw-thread">
        <span
          style={{ width: `${String(Math.round((remaining / status.idleTimeoutMs) * 100))}%` }}
        />
      </span>
      <span aria-hidden="true" className="tvv-clock">
        {clock}
      </span>
    </button>
  );
}

function SecretRow({
  row,
  onChanged,
  context,
}: {
  row: Snippet;
  onChanged: () => Promise<void>;
  context: { open: (event: React.MouseEvent, items: readonly MenuEntry[]) => void };
}) {
  const tr = useTr();
  const [revealed, setRevealed] = useState<string | null>(null);
  const [ttl, setTtl] = useState(0);
  const [copied, setCopied] = useState<number | null>(null);
  const [failedAction, setFailedAction] = useState(false);
  const [editing, setEditing] = useState<{ title: string; body: string } | null>(null);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const tick = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearReveal = useCallback(() => {
    if (hideTimer.current !== null) clearTimeout(hideTimer.current);
    if (tick.current !== null) clearInterval(tick.current);
    setRevealed(null);
    setTtl(0);
  }, []);

  useEffect(() => () => clearReveal(), [clearReveal]);

  const reveal = useCallback(async () => {
    setFailedAction(false);
    try {
      const text = await vaultReveal(row.id);
      setRevealed(text);
      setTtl(REVEAL_TTL_MS / 1000);
      if (hideTimer.current !== null) clearTimeout(hideTimer.current);
      if (tick.current !== null) clearInterval(tick.current);
      hideTimer.current = setTimeout(clearReveal, REVEAL_TTL_MS);
      tick.current = setInterval(() => setTtl((left) => Math.max(0, left - 1)), 1000);
    } catch {
      setFailedAction(true);
    }
  }, [row.id, clearReveal]);

  // Editing is the one place plaintext is held in a field: decrypt into the
  // form, then drop it from state on save or cancel.
  const startEdit = useCallback(async () => {
    setFailedAction(false);
    clearReveal();
    try {
      setEditing({ title: row.title, body: await vaultReveal(row.id) });
    } catch {
      setFailedAction(true);
    }
  }, [row.id, row.title, clearReveal]);

  const copy = useCallback(async () => {
    setFailedAction(false);
    try {
      setCopied(await panelCopySecret(row.id));
    } catch {
      setFailedAction(true);
    }
  }, [row.id]);

  useEffect(() => {
    if (copied === null) return;
    const timer = setTimeout(() => setCopied(null), 1600);
    return () => clearTimeout(timer);
  }, [copied]);

  const menu: MenuEntry[] = [
    { label: tr('Edit', '编辑'), onSelect: () => void startEdit() },
    'divider',
    {
      label: tr('Move to Trash', '移到回收站'),
      danger: true,
      onSelect: () => {
        void trashSnippet(row.id).then(onChanged);
      },
    },
  ];

  if (editing !== null) {
    return (
      <SecretForm
        initial={editing}
        submitLabel={tr('Save', '保存')}
        onCancel={() => setEditing(null)}
        onSave={async (title, body) => {
          await vaultUpdateSecret({
            id: row.id,
            title,
            body,
            snippetType: row.snippetType,
            description: row.description,
            folderId: row.folderId,
            trigger: row.trigger,
            triggerMode: row.triggerMode,
            language: row.language,
            isFavorite: row.isFavorite,
            isPinned: row.isPinned,
            isEnabled: row.isEnabled,
          });
          setEditing(null);
          await onChanged();
        }}
      />
    );
  }

  return (
    <Row className="tvv-row" label={row.title} onContextMenu={(event) => context.open(event, menu)}>
      <TypeMark code="SC" />
      <span className="tvv-value">
        <span className="tvw-row-title">{row.title}</span>
        {revealed === null ? (
          <span className="tvv-secret-word">{tr('Secret', '密文')}</span>
        ) : (
          <code className="tvv-plain">{revealed}</code>
        )}
      </span>
      <span className="tvv-value">
        {failedAction && <span className="tvv-note">{tr('try again', '请重试')}</span>}
        {copied !== null && (
          <span className="tvv-note">
            {tr(
              `Clipboard clears in ${String(Math.round(copied / 1000))}s`,
              `剪贴板 ${String(Math.round(copied / 1000))} 秒后清除`,
            )}
          </span>
        )}
        {revealed !== null && <span className="tvv-ttl">{`${String(ttl)}s`}</span>}
        <span className="tvw-hover-actions">
          {revealed === null ? (
            <Action label={tr('Reveal', '显示')} onRun={() => void reveal()} />
          ) : (
            <Action label={tr('Hide', '隐藏')} onRun={clearReveal} />
          )}
          <button
            type="button"
            className={`tvw-act ${copied === null ? '' : 'is-done'}`}
            onClick={() => void copy()}
          >
            {copied === null ? tr('Copy', '复制') : tr('Copied', '已复制')}
          </button>
          <OverflowMenu label={tr('More actions', '更多操作')} items={menu} />
        </span>
      </span>
    </Row>
  );
}

function SecretForm({
  initial,
  submitLabel,
  onCancel,
  onSave,
}: {
  initial?: { title: string; body: string };
  submitLabel: string;
  onCancel: () => void;
  onSave: (title: string, body: string) => Promise<void>;
}) {
  const tr = useTr();
  const [title, setTitle] = useState(initial?.title ?? '');
  const [body, setBody] = useState(initial?.body ?? '');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (busy) return;
      if (title.trim() === '' || body === '') {
        setError(tr('A title and a secret value are both required.', '标题与密文内容均为必填。'));
        return;
      }
      setBusy(true);
      setError(null);
      try {
        await onSave(title.trim(), body);
        setBody('');
      } catch (raw) {
        setError(
          raw instanceof IpcError
            ? tr(...ipcErrorCopy(raw))
            : tr('could not save the secret', '未能保存密文'),
        );
      } finally {
        setBusy(false);
      }
    },
    [title, body, busy, onSave, tr],
  );

  return (
    <form className="tvv-form" onSubmit={submit}>
      <label className="tvv-field">
        <span>{tr('Title', '标题')}</span>
        <input
          type="text"
          aria-label={tr('Secret title', '密文标题')}
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          disabled={busy}
        />
      </label>
      <label className="tvv-field">
        <span>{tr('Secret', '密文内容')}</span>
        <textarea
          rows={3}
          aria-label={tr('Secret value', '密文内容')}
          value={body}
          onChange={(event) => setBody(event.target.value)}
          disabled={busy}
        />
      </label>
      <div className="tvv-form-actions">
        <Action
          label={tr('Cancel', '取消')}
          onRun={() => {
            setBody('');
            onCancel();
          }}
        />
        <button type="submit" className="tvw-chip" disabled={busy}>
          {submitLabel}
        </button>
      </div>
      {error !== null && (
        <p className="tvv-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

/** Offered once, then it lives in Security and stops asking. */
function TouchIdOffer() {
  const tr = useTr();
  const [asked, setAsked] = useState(() => localStorage.getItem(TOUCH_ID_ASKED_KEY) === 'true');
  const [state, setState] = useState<'idle' | 'done' | 'error'>('idle');

  const remember = () => {
    localStorage.setItem(TOUCH_ID_ASKED_KEY, 'true');
    setAsked(true);
  };

  if (asked || state === 'done') return null;
  return (
    <div className="tvv-offer">
      <span>{tr('Use Touch ID next time?', '下次用 Touch ID 解锁?')}</span>
      <Action
        label={tr('Enable', '启用')}
        cta
        onRun={() => {
          vaultEnableBiometric()
            .then(() => {
              setState('done');
              remember();
            })
            .catch(() => setState('error'));
        }}
      />
      <Action label={tr('Not now', '不用了')} onRun={remember} />
      {state === 'error' && (
        <span className="tvv-note">
          {tr(
            'Touch ID is unavailable here — the master password still works.',
            '此处暂时无法使用 Touch ID——主密码仍然可用。',
          )}
        </span>
      )}
    </div>
  );
}

/** The same enrolment, reachable for good from inside Security. */
function BiometricSetting() {
  const tr = useTr();
  const [state, setState] = useState<'idle' | 'done' | 'error'>('idle');
  return (
    <p className="tvv-policy-item">
      <span aria-hidden="true" className="tvv-square" />
      <span>
        {state === 'done'
          ? tr('Touch ID is on for this vault.', '此保险库已开启 Touch ID 解锁。')
          : tr('Touch ID can unlock this vault.', 'Touch ID 可用于解锁此保险库。')}
        {state !== 'done' && (
          <Action
            label={tr('Enable Touch ID', '开启 Touch ID')}
            onRun={() => {
              vaultEnableBiometric()
                .then(() => setState('done'))
                .catch(() => setState('error'));
            }}
          />
        )}
        {state === 'error' && (
          <span className="tvv-note">
            {tr(
              'Touch ID is unavailable here — the master password still works.',
              '此处暂时无法使用 Touch ID——主密码仍然可用。',
            )}
          </span>
        )}
      </span>
    </p>
  );
}

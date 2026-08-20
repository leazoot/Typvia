import { vaultUnlockBiometric, vaultUnlockPassword, type Snippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';

interface PanelVerifyProps {
  snippet: Snippet;
  action: 'insert' | 'copy';
  destination: string | null;
  /** Called once an unlock succeeds — the parent then performs the action. */
  onUnlocked: () => void;
  onCancel: () => void;
}

/**
 * Panel sensitive-verification entry. Calling a vault snippet from
 * the panel requires an unlocked vault: this replaces the results list with the
 * unlock prompt (Touch ID + master password) — the same sanctioned vault-unlock
 * surface as the main window, rendered inline in the panel. A failed unlock is
 * undifferentiated (never says which part was wrong) and leads with what is
 * still safe. ESC returns to the list.
 */
export function PanelVerify({
  snippet,
  action,
  destination,
  onUnlocked,
  onCancel,
}: PanelVerifyProps) {
  const tr = useTr();
  const [password, setPassword] = useState('');
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);

  const unlock = async (run: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setFailed(false);
    try {
      await run();
      onUnlocked();
    } catch {
      // Undifferentiated by design: never reveal which factor was wrong.
      setFailed(true);
      setBusy(false);
    }
  };

  const submitPassword = () => {
    if (password.trim() === '') return;
    void unlock(() => vaultUnlockPassword(password));
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      submitPassword();
    }
  };

  return (
    <div className="tv-panel-verify" onKeyDown={onKeyDown}>
      <div className="tv-panel-verify-head">
        <span className="tv-panel-verify-title">
          {tr(
            `Unlock to ${action === 'insert' ? 'insert' : 'copy'} · ${snippet.title}`,
            `解锁以${action === 'insert' ? '插入' : '复制'} · ${snippet.title}`,
          )}
        </span>
        {destination !== null && <span className="tv-panel-dest">→ {destination}</span>}
      </div>
      <p className="tv-panel-verify-sub">
        {tr(
          `Unlock the vault to ${action === 'insert' ? 'insert' : 'copy'} this secret.`,
          `解锁保险库后${action === 'insert' ? '插入' : '复制'}此密文。`,
        )}
      </p>

      <button
        type="button"
        className="tv-panel-verify-touch"
        disabled={busy}
        onClick={() => void unlock(() => vaultUnlockBiometric())}
      >
        {tr('Use Touch ID', '使用 Touch ID')}
      </button>

      <label className="tv-panel-verify-field">
        <span className="tv-panel-verify-key">{tr('Master password', '主密码')}</span>
        <input
          type="password"
          className="tv-panel-verify-input"
          aria-label={tr('Master password', '主密码')}
          value={password}
          autoFocus
          onChange={(event) => setPassword(event.target.value)}
        />
      </label>

      <div className="tv-panel-verify-actions">
        <button
          type="button"
          className="tv-panel-verify-go"
          disabled={busy || password.trim() === ''}
          onClick={submitPassword}
        >
          {tr('Unlock', '解锁')}
        </button>
      </div>

      {/* Generic in both languages: never says which factor was wrong. */}
      {failed && (
        <p className="tv-panel-verify-error" role="alert">
          {tr(
            'Your vault is still safe — that didn’t unlock it. Try again.',
            '你的保险库仍然安全——这次尝试未能解锁，请重试。',
          )}
        </p>
      )}

      <footer className="tv-panel-footer">
        <span className="tv-panel-keys">
          <kbd>↵</kbd>
          <span className="tv-panel-key-word">{tr('unlock', '解锁')}</span>
          <kbd>esc</kbd>
          <span className="tv-panel-key-word">{tr('back', '返回')}</span>
        </span>
      </footer>
    </div>
  );
}

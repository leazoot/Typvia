// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { IpcError, ipcErrorCopy, type Snippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState, type FormEvent, type MouseEvent } from 'react';
import { TextAction } from '../../paper/kit';

/** Every secret is masked to the same length: the mask must not hint at the value. */
const MASK = '••••••••••';

function quiet(run: () => void) {
  return (event: MouseEvent) => {
    event.stopPropagation();
    run();
  };
}

/**
 * One secret: trigger and title on the first line, the masked value on the
 * second. "Peek" shows the plaintext for a few seconds and then covers it
 * again by itself; copy and edit surface under the pointer.
 */
export function SecretRow({
  row,
  shown,
  copiedSeconds,
  onPeek,
  onCover,
  onCopy,
  onEdit,
  onMenu,
}: {
  row: Snippet;
  /** The plaintext and the seconds it has left on screen, while peeking. */
  shown: { text: string; left: number } | null;
  /** Seconds until the clipboard clears, just after a copy. */
  copiedSeconds: number | null;
  onPeek: () => void;
  onCover: () => void;
  onCopy: () => void;
  onEdit: () => void;
  onMenu: (event: MouseEvent) => void;
}) {
  const tr = useTr();
  return (
    <div className="tvv-row" role="listitem" aria-label={row.title} onContextMenu={onMenu}>
      <div className="tvv-row-line">
        {row.trigger !== null && <span className="tvv-trigger">{row.trigger}</span>}
        <span className="tvv-title">{row.title}</span>
        <span className="tvv-grow" />
        <span className="tvv-row-more">
          <button type="button" className="tvv-word" onClick={quiet(onCopy)}>
            {tr('Copy', '复制')}
          </button>
          <button type="button" className="tvv-word" onClick={quiet(onEdit)}>
            {tr('Edit', '编辑')}
          </button>
        </span>
        {shown === null ? (
          <button type="button" className="tvv-word" onClick={quiet(onPeek)}>
            {tr('Peek', '看一眼')}
          </button>
        ) : (
          <button type="button" className="tvv-word" onClick={quiet(onCover)}>
            {tr('Cover', '遮回')}
          </button>
        )}
      </div>
      <div className="tvv-secret">
        {shown === null ? <span aria-hidden="true">{MASK}</span> : <code>{shown.text}</code>}
        {shown !== null && (
          <span className="tvv-secret-note">
            {tr(`covers again in ${String(shown.left)}s`, `${String(shown.left)} 秒后遮回`)}
          </span>
        )}
        {copiedSeconds !== null && (
          <span className="tvv-secret-note" role="status">
            {tr(
              `Clipboard clears in ${String(copiedSeconds)}s`,
              `剪贴板 ${String(copiedSeconds)} 秒后清掉`,
            )}
          </span>
        )}
      </div>
    </div>
  );
}

/**
 * Writing a secret. Editing is the one place plaintext sits in a field: it
 * is decrypted into the form and dropped from state on save or cancel.
 */
export function SecretForm({
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

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    if (busy) return;
    if (title.trim() === '' || body === '') {
      setError(tr('A title and a secret value are both needed.', '标题和密文内容都要填。'));
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
          : tr('The secret was not saved.', '密文没存上。'),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="tvv-form" onSubmit={(event) => void submit(event)}>
      <label className="tvv-field">
        <span className="tvv-field-label">{tr('Title', '标题')}</span>
        <span className="tvv-line">
          <input
            type="text"
            aria-label={tr('Secret title', '密文标题')}
            value={title}
            disabled={busy}
            onChange={(event) => setTitle(event.target.value)}
          />
        </span>
      </label>
      <label className="tvv-field">
        <span className="tvv-field-label">{tr('Secret', '密文')}</span>
        <textarea
          className="tvv-box"
          rows={3}
          aria-label={tr('Secret value', '密文内容')}
          value={body}
          disabled={busy}
          onChange={(event) => setBody(event.target.value)}
        />
      </label>
      {error !== null && (
        <p className="tvv-hint" role="alert">
          {error}
        </p>
      )}
      <div className="tvv-form-actions">
        <TextAction primary onClick={() => void submit()}>
          {submitLabel}
        </TextAction>
        <button
          type="button"
          className="tpi-note-no"
          onClick={() => {
            setBody('');
            onCancel();
          }}
        >
          {tr('Not now', '先别')}
        </button>
      </div>
    </form>
  );
}

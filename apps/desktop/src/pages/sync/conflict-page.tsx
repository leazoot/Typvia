// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Conflict resolution — two full columns instead of a
 * merged diff: a three-way merge editor is the right tool for code and the
 * wrong tool for a person's saved reply. Both versions are shown whole and
 * readable, and the choice is made by reading.
 *
 * "Decide later" is a first-class option: an unresolved conflict is not an
 * error, it is two devices that both still work.
 */
import type { Snippet } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { relativeTime, shapeLabel, useConflictDecision } from '@typvia/ui/sync';
import { useNavigate } from 'react-router';
import './sync.css';

export function ConflictPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const { pairs, busy, error, now, resolve } = useConflictDecision();

  if (pairs === null) {
    return (
      <main className="tv-sync-main" aria-busy="true">
        <div className="tv-sync-skeleton-line" />
        <div className="tv-sync-skeleton-line" />
      </main>
    );
  }

  const pair = pairs[0];
  if (pair === undefined) {
    return (
      <main className="tv-conflict">
        <h1 className="tv-sync-title">{tr('Nothing left to decide.', '没有需要处理的冲突了。')}</h1>
        <p className="tv-sync-subtitle">
          {tr('Every conflict has been resolved', '所有冲突都已处理')}
        </p>
        <div className="tv-conflict-actions">
          <button type="button" className="tv-sync-button" onClick={() => void navigate('/sync')}>
            {tr('Back to devices', '返回设备列表')}
          </button>
        </div>
      </main>
    );
  }

  return (
    <main className="tv-conflict">
      <div className="tv-conflict-count">
        <span aria-hidden="true" className="tv-sync-conflict-dot" />
        <span role="status">
          {pairs.length === 1
            ? tr('1 conflict', '1 个冲突')
            : tr(
                `1 of ${String(pairs.length)} conflicts`,
                `第 1 个，共 ${String(pairs.length)} 个冲突`,
              )}
        </span>
      </div>
      <h1 className="tv-sync-title">
        {tr(`${pair.source.title} changed in two places.`, `${pair.source.title} 在两处都被修改。`)}
      </h1>
      <p className="tv-sync-subtitle">
        {tr(
          'Both devices changed this one. No automatic merge — you decide.',
          '两台设备都改了这一条。没有自动合并 —— 由你决定。',
        )}
      </p>
      <p className="tv-conflict-lead">
        {tr(
          'Both edits happened before the two devices could hear about each other. Typvia does not guess with your text, so it kept both.',
          '两次编辑发生在两台设备互相知晓之前。Typvia 不会对你的文本做猜测，因此两个版本都保留了下来。',
        )}
      </p>

      <div className="tv-conflict-pair">
        <VersionColumn
          snippet={pair.source}
          role="source"
          sensitive={pair.sensitive}
          now={now}
          busy={busy}
          onKeep={() => void resolve(pair, 'source')}
        />
        <div className="tv-conflict-or" aria-hidden="true">
          <span className="tv-conflict-or-line" />
          <span>{tr('or', '或')}</span>
          <span className="tv-conflict-or-line" />
        </div>
        <VersionColumn
          snippet={pair.copy}
          role="copy"
          sensitive={pair.sensitive}
          now={now}
          busy={busy}
          onKeep={() => void resolve(pair, 'copy')}
        />
      </div>

      <div className="tv-conflict-actions">
        <button
          type="button"
          className="tv-sync-button"
          disabled={busy}
          onClick={() => void resolve(pair, 'both')}
        >
          {tr('Keep both as two snippets', '两个都保留为两条片段')}
        </button>
        <button
          type="button"
          className="tv-sync-button"
          onClick={() => void navigate(`/editor/${pair.copy.id}`)}
        >
          {tr('Open the other version in the editor', '在编辑器中打开另一个版本')}
        </button>
        <button type="button" className="tv-sync-link" onClick={() => void navigate('/sync')}>
          {tr('Decide later — both stay on their devices', '以后再处理 —— 两个版本各自留在设备上')}
        </button>
        <span className="tv-conflict-promise">
          {tr(
            'Whichever you pick, the other moves to the Trash. Nothing is deleted.',
            '无论你选哪一个，另一个都会移入回收站。不会删除任何内容。',
          )}
        </span>
      </div>
      {error !== null && (
        <p className="tv-conflict-lead" role="status">
          {error}
        </p>
      )}
    </main>
  );
}

interface ColumnProps {
  snippet: Snippet;
  role: 'source' | 'copy';
  sensitive: boolean;
  now: number;
  busy: boolean;
  onKeep: () => void;
}

function VersionColumn({ snippet, role, sensitive, now, busy, onKeep }: ColumnProps) {
  const tr = useTr();
  const locale = useLocale();
  // The entity kept one body; the other was parked as a copy. Naming them by
  // role rather than by device is the honest choice: the record says which
  // body won the entity, not which device typed it.
  const label =
    role === 'source'
      ? tr('The version in use', '正在使用的版本')
      : tr('The version set aside', '被搁置的版本');
  return (
    <section
      className={`tv-conflict-column${role === 'source' ? ' tv-conflict-column-source' : ''}`}
      aria-label={label}
    >
      <header className="tv-conflict-head">
        <div className="tv-conflict-head-row">
          <span aria-hidden="true" className="tv-conflict-mark" />
          <span className="tv-conflict-name">{label}</span>
          <span className="tv-conflict-time">{relativeTime(snippet.updatedAt, now, locale)}</span>
        </div>
        <div className="tv-conflict-shape">{shapeLabel(snippet, sensitive, locale)}</div>
      </header>
      {sensitive || snippet.body === null ? (
        <p className="tv-conflict-body tv-conflict-locked">
          {tr(
            'This is a secret, so both versions stay encrypted while you compare them. Keep the one from the device you trust for this edit — the other keeps its own copy in the Trash.',
            '这是机密内容，因此比较时两个版本都保持加密。保留你信任这次编辑的那台设备上的版本 —— 另一个版本会在回收站中保留它自己的副本。',
          )}
        </p>
      ) : (
        <pre className="tv-conflict-body" aria-hidden="true">
          {snippet.body}
        </pre>
      )}
      <div className="tv-conflict-foot">
        <button type="button" className="tv-conflict-keep" disabled={busy} onClick={onKeep}>
          {tr('Keep this one', '保留这一个')}
        </button>
      </div>
    </section>
  );
}

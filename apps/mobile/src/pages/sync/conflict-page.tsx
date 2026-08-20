/**
 * Conflict arbitration on a phone. The desktop design puts the two versions
 * side by side; two columns do not survive 390pt, so they stack here and both
 * bodies stay fully readable — the thesis (read both, then choose; never a
 * three-way merge) is unchanged — a deliberate design deviation.
 *
 * "Decide later" stays a first-class action: an unresolved conflict is not an
 * error, it is two devices that both still work.
 */
import type { Snippet } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { conflictHeadline, relativeTime, shapeLabel, useConflictDecision } from '@typvia/ui/sync';
import { ScreenSkeleton, SettingsScreen } from '../settings/screen';
import './sync.css';

export function MobileConflictPage({ onBack }: { onBack: () => void }) {
  const tr = useTr();
  const locale = useLocale();
  const { pairs, busy, error, now, resolve } = useConflictDecision();

  if (pairs === null) return <ScreenSkeleton parent={tr('Sync', '同步')} onBack={onBack} />;

  const pair = pairs[0];
  if (pair === undefined) {
    return (
      <SettingsScreen
        parent={tr('Sync', '同步')}
        title={tr('Nothing left to decide.', '没有需要处理的冲突了。')}
        onBack={onBack}
      >
        <button type="button" className="tv-mset-button" onClick={onBack}>
          {tr('Back to sync', '返回同步')}
        </button>
      </SettingsScreen>
    );
  }

  return (
    <SettingsScreen
      parent={tr('Sync', '同步')}
      title={tr(
        `${pair.source.title} changed in two places.`,
        `「${pair.source.title}」在两处被修改。`,
      )}
      intro={tr(
        'Both edits happened before the two devices could hear about each other. Typvia does not guess with your text, so it kept both.',
        '两次编辑都发生在两台设备互相得知对方之前。Typvia 不对你的文本做猜测,所以两个版本都保留了。',
      )}
      onBack={onBack}
    >
      <div className="tv-sync-rail-label" role="status">
        {conflictHeadline(pairs.length, locale)}
      </div>

      <VersionCard
        snippet={pair.source}
        role="source"
        sensitive={pair.sensitive}
        now={now}
        busy={busy}
        onKeep={() => void resolve(pair, 'source')}
      />
      <VersionCard
        snippet={pair.copy}
        role="copy"
        sensitive={pair.sensitive}
        now={now}
        busy={busy}
        onKeep={() => void resolve(pair, 'copy')}
      />

      <button
        type="button"
        className="tv-mset-button"
        disabled={busy}
        onClick={() => void resolve(pair, 'both')}
      >
        {tr('Keep both as two snippets', '两个都保留为两条片段')}
      </button>
      <button type="button" className="tv-mset-button" onClick={onBack}>
        {tr('Decide later — both stay on their devices', '稍后决定 —— 两个版本各自留在设备上')}
      </button>
      <span className="tv-msync-promise">
        {tr(
          'Whichever you pick, the other moves to the Trash. Nothing is deleted.',
          '无论选哪个,另一个都会移入回收站。不会删除任何内容。',
        )}
      </span>
      {error !== null && (
        <span className="tv-msync-notice" role="status">
          {error}
        </span>
      )}
    </SettingsScreen>
  );
}

interface CardProps {
  snippet: Snippet;
  role: 'source' | 'copy';
  sensitive: boolean;
  now: number;
  busy: boolean;
  onKeep: () => void;
}

function VersionCard({ snippet, role, sensitive, now, busy, onKeep }: CardProps) {
  const tr = useTr();
  const locale = useLocale();
  // The entity kept one body; the other was parked as a copy. Naming them by
  // role rather than by device is the honest choice: the record says which
  // body won the entity, not which device typed it.
  const label =
    role === 'source'
      ? tr('The version in use', '当前使用的版本')
      : tr('The version set aside', '被搁置的版本');
  return (
    <section
      className={role === 'source' ? 'tv-msync-version is-source' : 'tv-msync-version'}
      aria-label={label}
    >
      <div className="tv-msync-version-head">
        <span className="tv-msync-version-name">{label}</span>
        <span className="tv-msync-version-time">
          {relativeTime(snippet.updatedAt, now, locale)}
        </span>
      </div>
      <div className="tv-msync-version-shape">{shapeLabel(snippet, sensitive, locale)}</div>
      {sensitive || snippet.body === null ? (
        <p className="tv-msync-version-locked">
          {tr(
            'This is a secret, so both versions stay encrypted while you compare them. Keep the one from the device you trust for this edit — the other keeps its own copy in the Trash.',
            '这是一条秘密,所以比较时两个版本都保持加密。保留你信任这次编辑的那台设备上的版本 —— 另一个版本会在回收站里保留自己的副本。',
          )}
        </p>
      ) : (
        <pre className="tv-msync-version-body" aria-hidden="true">
          {snippet.body}
        </pre>
      )}
      <button type="button" className="tv-mset-button" disabled={busy} onClick={onKeep}>
        {tr('Keep this one', '保留这个')}
      </button>
    </section>
  );
}

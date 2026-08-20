/**
 * Sync & devices — "Know that my content is safely on the devices I expect,
 * and only those."
 *
 * The page is a drawing, never a management table. It carries seven states:
 * synced · syncing · offline (dashed line) · conflict (amber node) · new
 * device pairing · device revoked (struck node) · self-hosted server, plus
 * the loading and first-use states the 17-state system requires.
 *
 * The state machine behind it is shared with the mobile host
 * (`@typvia/ui/sync`); only the drawing and the rail are desktop's own.
 */
import { useLocale, useTr } from '@typvia/ui';
import { conflictHeadline, headline, revokeConfirmation, useSyncOverview } from '@typvia/ui/sync';
import { useNavigate } from 'react-router';
import { SyncRail } from './rail';
import { SyncRoute } from './route';
import { SyncSetup } from './setup';
import './sync.css';

export function SyncPage() {
  const tr = useTr();
  const locale = useLocale();
  const navigate = useNavigate();
  const sync = useSyncOverview();
  const { status, devices, conflicts, offline, running, settling, notice, busy } = sync;

  if (status === null) {
    return (
      <main className="tv-sync-main" aria-busy="true">
        <div className="tv-sync-skeleton-line" />
        <div className="tv-sync-skeleton-line" />
        <div className="tv-sync-skeleton-line" />
      </main>
    );
  }

  if (!status.configured) {
    return <SyncSetup status={status} onDone={() => void sync.refresh()} />;
  }

  const confirmRevoke = sync.confirmRevoke;

  return (
    <div className="tv-sync">
      <main className="tv-sync-main">
        <h1 className="tv-sync-title">{headline(devices.length, status.enabled, locale)}</h1>
        <p className="tv-sync-subtitle">
          {tr(
            'End-to-end encrypted · the server stores only ciphertext',
            '端到端加密 · 服务器只存密文',
          )}
          {status.pendingBacklog > 0
            ? tr(
                ` · ${String(status.pendingBacklog)} to send`,
                ` · ${String(status.pendingBacklog)} 条待发送`,
              )
            : ''}
        </p>

        {status.recoveryCatchupPending && (
          <p className="tv-sync-subtitle" role="status">
            {tr(
              'Recovery is in — the history catch-up finishes in the background and resumes on its own.',
              '恢复已完成——历史记录正在后台补拉,中断后会自动继续。',
            )}
          </p>
        )}

        <SyncRoute
          devices={devices}
          running={running}
          animating={settling && !offline}
          offline={offline}
          conflictCount={conflicts.length}
          lastSyncAt={status.lastSyncAt}
          now={sync.now}
          onPairDevice={() => void navigate('/sync/pair')}
          onRevoke={sync.askRevoke}
          onPass={sync.endPass}
        />

        {confirmRevoke !== null && (
          <div
            className="tv-sync-conflict-card"
            role="group"
            aria-label={tr('Confirm revoke', '确认撤销')}
          >
            <p className="tv-sync-conflict-title">
              {revokeConfirmation(confirmRevoke.name, locale)}
            </p>
            <div className="tv-sync-actions">
              <button
                type="button"
                className="tv-sync-danger"
                disabled={busy}
                onClick={() => void sync.revoke(confirmRevoke)}
              >
                {tr(`Revoke ${confirmRevoke.name}`, `撤销 ${confirmRevoke.name}`)}
              </button>
              <button
                type="button"
                className="tv-sync-button"
                onClick={() => {
                  sync.askRevoke(null);
                }}
              >
                {tr('Keep it', '保留')}
              </button>
            </div>
          </div>
        )}

        {conflicts.length > 0 && (
          <div className="tv-sync-conflict-card">
            <div className="tv-sync-conflict-head">
              <span aria-hidden="true" className="tv-sync-conflict-dot" />
              <span className="tv-sync-conflict-title">
                {conflictHeadline(conflicts.length, locale)}
              </span>
              <span className="tv-sync-conflict-trigger">{conflicts[0]?.source.trigger ?? ''}</span>
            </div>
            <div className="tv-sync-actions">
              <button
                type="button"
                className="tv-sync-primary"
                onClick={() => void navigate('/sync/conflicts')}
              >
                {tr('Decide now', '现在处理')}
              </button>
              <span className="tv-sync-row-meta">
                {tr(
                  'Both versions are kept until you choose. Nothing is deleted.',
                  '在你做出选择之前，两个版本都会保留。不会删除任何内容。',
                )}
              </span>
            </div>
          </div>
        )}

        <div className="tv-sync-actions tv-sync-run-row">
          <button
            type="button"
            className="tv-sync-button"
            disabled={running || !status.enabled}
            onClick={() => void sync.runSync()}
          >
            {tr('Sync now', '立即同步')}
          </button>
          {!status.enabled && (
            <button
              type="button"
              className="tv-sync-primary"
              disabled={busy}
              onClick={() => void sync.resume()}
            >
              {tr('Turn sync back on', '重新开启同步')}
            </button>
          )}
          {notice !== null && (
            <span className="tv-sync-row-meta" role="status">
              {notice}
            </span>
          )}
        </div>
      </main>

      <SyncRail
        status={status}
        now={sync.now}
        busy={busy}
        onPairDevice={() => void navigate('/sync/pair')}
        onExportRecovery={() => void navigate('/sync/recovery')}
        onRotateKey={() => void sync.rotate()}
      />
    </div>
  );
}

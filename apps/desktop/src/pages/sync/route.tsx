/**
 * The route drawing. Every device row IS a segment of
 * the route: this device is a filled square on the left, every other device
 * sits at the far end of its own line, and only the line that is actually
 * moving carries the travelling segment.
 */
import type { SyncDevice } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { deviceRowMeta, selfStatus } from '@typvia/ui/sync';

interface RouteProps {
  devices: SyncDevice[];
  /** A round is in flight right now (drives the status word). */
  running: boolean;
  /** Paint the travelling segment: stays true past `running` so the pass
   * finishes rather than cutting, and false while offline (there is nothing
   * travelling on a line that cannot be reached). */
  animating: boolean;
  offline: boolean;
  conflictCount: number;
  lastSyncAt: number | null;
  now: number;
  onPairDevice: () => void;
  onRevoke: (device: SyncDevice) => void;
  /** Fires at the end of each pass so the caller can stop a finished loop. */
  onPass: () => void;
}

export function SyncRoute({
  devices,
  running,
  animating,
  offline,
  conflictCount,
  lastSyncAt,
  now,
  onPairDevice,
  onRevoke,
  onPass,
}: RouteProps) {
  const tr = useTr();
  return (
    <ul className="tv-sync-route">
      {devices.map((device) => (
        <DeviceRow
          key={device.deviceId}
          device={device}
          running={running}
          animating={animating}
          offline={offline}
          conflictCount={conflictCount}
          lastSyncAt={lastSyncAt}
          now={now}
          onRevoke={onRevoke}
          onPass={onPass}
        />
      ))}
      <li className="tv-sync-row">
        <div className="tv-sync-row-name">
          <span className="tv-sync-row-title tv-sync-row-meta">
            {tr('Pair a new device', '配对新设备')}
          </span>
          <span className="tv-sync-row-meta">{tr('scan a code · 60 seconds', '扫码 · 60 秒')}</span>
        </div>
        <span aria-hidden="true" className="tv-sync-node tv-sync-node-pending" />
        <span aria-hidden="true" className="tv-sync-line tv-sync-line-quiet" />
        <button type="button" className="tv-sync-row-action" onClick={onPairDevice}>
          {tr('Show code', '显示配对码')}
        </button>
      </li>
    </ul>
  );
}

interface RowProps {
  device: SyncDevice;
  running: boolean;
  animating: boolean;
  offline: boolean;
  conflictCount: number;
  lastSyncAt: number | null;
  now: number;
  onRevoke: (device: SyncDevice) => void;
  onPass: () => void;
}

function DeviceRow({
  device,
  running,
  animating,
  offline,
  conflictCount,
  lastSyncAt,
  now,
  onRevoke,
  onPass,
}: RowProps) {
  const tr = useTr();
  const locale = useLocale();
  const revoked = device.revokedAt !== null;
  // Conflicts are awaiting a decision here, on this device — the record does
  // not say which device authored the other body, so the marker sits on the
  // line that owns the decision rather than guessing an attribution.
  const conflicted = device.isThisDevice && conflictCount > 0;
  const nodeClass = [
    'tv-sync-node',
    device.isThisDevice ? 'tv-sync-node-self' : '',
    revoked ? 'tv-sync-node-revoked' : '',
    conflicted ? 'tv-sync-node-conflict' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <li className="tv-sync-row">
      <div className="tv-sync-row-name">
        <span className="tv-sync-row-title">{device.name}</span>
        <span className={`tv-sync-row-meta${conflicted ? ' tv-sync-row-meta-warning' : ''}`}>
          {deviceRowMeta(device, conflictCount, now, locale)}
        </span>
      </div>
      <span aria-hidden="true" className={nodeClass} />
      <span
        aria-hidden="true"
        className={`tv-sync-line${offline && !revoked ? ' tv-sync-line-offline' : ''}${
          revoked ? ' tv-sync-line-quiet' : ''
        }`}
      >
        {device.isThisDevice && animating && !offline && (
          <span className="tv-sync-segment" onAnimationIteration={onPass} />
        )}
        {conflicted && <span className="tv-sync-line-marker" />}
      </span>
      {revoked ? (
        <span className="tv-sync-row-status">{tr('revoked', '已撤销')}</span>
      ) : device.isThisDevice ? (
        <span className={`tv-sync-row-status${running ? ' tv-sync-row-status-live' : ''}`}>
          {selfStatus(running, offline, lastSyncAt, now, locale)}
        </span>
      ) : (
        <button
          type="button"
          className="tv-sync-row-action tv-sync-danger"
          onClick={() => {
            onRevoke(device);
          }}
        >
          {tr('Revoke', '撤销')}
        </button>
      )}
    </li>
  );
}

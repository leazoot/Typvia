// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Mobile Sync screen. The phone design draws exactly one sync surface — a
 * `Sync` row with a dot, a device count and a chevron — and nothing behind
 * it; the screen this chevron opens is a deviation. It is built from the
 * defined settings grammar (section label → bordered group of 52pt rows →
 * footnote) rather than by shrinking the desktop route drawing, which is
 * fixed-width geometry.
 *
 * Every sentence, every derived label and the whole state machine come from
 * `@typvia/ui/sync`, shared with the desktop page.
 */
import type { SyncDevice } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import {
  backlogLabel,
  conflictCountLabel,
  deviceRowMeta,
  headline,
  revokeConfirmation,
  selfStatus,
  syncBadge,
  useSyncOverview,
} from '@typvia/ui/sync';
import { SettingsGroup, SettingsRow, SettingsToggle } from '../settings/rows';
import { ScreenSkeleton, SettingsScreen } from '../settings/screen';
import { SyncSetup } from './setup';
import './sync.css';

interface SyncPageProps {
  onBack: () => void;
  onPair: () => void;
  onConflicts: () => void;
  onRecovery: () => void;
  onRecover: () => void;
}

export function MobileSyncPage({
  onBack,
  onPair,
  onConflicts,
  onRecovery,
  onRecover,
}: SyncPageProps) {
  const tr = useTr();
  const locale = useLocale();
  const sync = useSyncOverview();
  const { status, devices, conflicts, offline, running, busy, notice, confirmRevoke } = sync;

  if (status === null) return <ScreenSkeleton parent={tr('Settings', '设置')} onBack={onBack} />;

  if (!status.configured) {
    return (
      <SyncSetup
        status={status}
        onBack={onBack}
        onDone={() => void sync.refresh()}
        onJoin={onPair}
        onRecover={onRecover}
      />
    );
  }

  const badge = syncBadge(status, locale);

  return (
    <SettingsScreen
      parent={tr('Settings', '设置')}
      title={tr('Sync', '同步')}
      intro={headline(devices.length, status.enabled, locale)}
      onBack={onBack}
    >
      <SettingsGroup label={tr('This account', '本账户')} annotated>
        <SettingsRow
          label={
            status.enabled
              ? tr('Syncing with your server', '正在与你的服务器同步')
              : tr('Sync is off', '同步已关闭')
          }
          sub={
            status.enabled
              ? tr(
                  'Changes on this device are encrypted and queued to send.',
                  '本机更改会加密后排队发送。',
                )
              : tr('The queue is kept — turn it back on any time.', '队列保留,随时可以重新开启。')
          }
          control={
            <SettingsToggle
              label={
                status.enabled ? tr('Turn sync off', '关闭同步') : tr('Turn sync on', '开启同步')
              }
              pressed={status.enabled}
              disabled={busy}
              onToggle={() => void sync.toggleEnabled()}
            />
          }
        />
        <SettingsRow
          label={tr('Server', '服务器')}
          sub={
            <span className="tv-mset-row-mono">{status.serverUrl ?? tr('not set', '未设置')}</span>
          }
          dot={badge.kind}
          value={badge.label}
        />
        <SettingsRow
          label={backlogLabel(status.pendingBacklog, locale)}
          sub={notice ?? selfStatus(running, offline, status.lastSyncAt, sync.now, locale)}
          control={
            <button
              type="button"
              className="tv-mset-action"
              disabled={running || !status.enabled}
              onClick={() => void sync.runSync()}
            >
              {tr('Sync now', '立即同步')}
            </button>
          }
        />
        {status.recoveryCatchupPending && (
          <SettingsRow
            label={tr('Recovery catch-up', '恢复补拉')}
            sub={tr(
              'Recovery is in — the history catch-up finishes in the background and resumes on its own.',
              '恢复已完成——历史记录正在后台补拉,中断后会自动继续。',
            )}
          />
        )}
        {conflicts.length > 0 && (
          <SettingsRow
            label={conflictCountLabel(conflicts.length, locale)}
            sub={tr(
              'Both versions are kept until you choose. Nothing is deleted.',
              '两个版本都会保留,直到你做出选择。不会删除任何内容。',
            )}
            dot="warning"
            onPress={onConflicts}
          />
        )}
      </SettingsGroup>
      <p className="tv-mset-note">
        {tr(
          `Key generation ${String(status.keyGeneration)} · account ${status.accountId ?? 'unknown'}. The server stores ciphertext and never holds a key, so it can never read a snippet.`,
          `密钥代次 ${String(status.keyGeneration)} · 账户 ${status.accountId ?? '未知'}。服务器只存密文、从不持有密钥,因此永远读不到任何片段。`,
        )}
      </p>

      <SettingsGroup label={tr('Devices', '设备')}>
        {devices.map((device) => (
          <SettingsRow
            key={device.deviceId}
            label={device.name}
            sub={deviceRowMeta(device, conflicts.length, sync.now, locale)}
            {...(device.isThisDevice
              ? { value: selfStatus(running, offline, status.lastSyncAt, sync.now, locale) }
              : { control: <RevokeAction device={device} onAsk={sync.askRevoke} /> })}
          />
        ))}
        <SettingsRow
          label={tr('Pair a device', '配对新设备')}
          sub={tr('scan a code · 60 seconds', '扫码配对 · 60 秒')}
          onPress={onPair}
        />
      </SettingsGroup>

      {confirmRevoke !== null && (
        <div
          className="tv-msync-confirm"
          role="group"
          aria-label={tr('Confirm revoke', '确认吊销')}
        >
          <p className="tv-msync-confirm-text">{revokeConfirmation(confirmRevoke.name, locale)}</p>
          <button
            type="button"
            className="tv-mset-button"
            disabled={busy}
            onClick={() => void sync.revoke(confirmRevoke)}
          >
            {tr(`Revoke ${confirmRevoke.name}`, `吊销 ${confirmRevoke.name}`)}
          </button>
          <button
            type="button"
            className="tv-mset-button"
            onClick={() => {
              sync.askRevoke(null);
            }}
          >
            {tr('Keep it', '保留')}
          </button>
        </div>
      )}

      <SettingsGroup label={tr('If every device is lost', '所有设备都丢失时')}>
        <SettingsRow
          label={tr('Recovery code', '恢复码')}
          sub={
            status.recoveryExportedAt === null
              ? tr('No recovery code exported from this device.', '这台设备还没有导出过恢复码。')
              : tr('Exported from this device.', '已从这台设备导出。')
          }
          onPress={onRecovery}
        />
        <SettingsRow
          label={tr('Rotate the sync key', '轮换同步密钥')}
          sub={tr(
            'New snippets use a new key; existing ones are unchanged.',
            '新片段使用新密钥;已有片段保持不变。',
          )}
          control={
            <button
              type="button"
              className="tv-mset-action"
              disabled={busy}
              onClick={() => void sync.rotate()}
            >
              {tr('Rotate', '轮换')}
            </button>
          }
        />
      </SettingsGroup>
    </SettingsScreen>
  );
}

/** Destructive action: a red label on a tinted hairline, never a red fill,
 * and it only asks — the confirmation names the device before anything runs. */
function RevokeAction({
  device,
  onAsk,
}: {
  device: SyncDevice;
  onAsk: (device: SyncDevice) => void;
}) {
  const tr = useTr();
  if (device.revokedAt !== null) {
    return <span className="tv-mset-row-value">{tr('revoked', '已吊销')}</span>;
  }
  return (
    <button
      type="button"
      className="tv-mset-action is-danger"
      onClick={() => {
        onAsk(device);
      }}
    >
      {tr('Revoke', '吊销')}
    </button>
  );
}

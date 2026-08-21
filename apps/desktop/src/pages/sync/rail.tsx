// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The Sync page's right rail: where the account syncs,
 * and the two things you only do occasionally — export a recovery code, and
 * rotate the key. Pairing itself is a page, not a panel step, because it
 * spans two devices and a comparison the user must actually make.
 */
import type { SyncStatus } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { ageLabel } from '@typvia/ui/sync';

interface RailProps {
  status: SyncStatus;
  now: number;
  busy: boolean;
  onPairDevice: () => void;
  onExportRecovery: () => void;
  onRotateKey: () => void;
}

export function SyncRail({
  status,
  now,
  busy,
  onPairDevice,
  onExportRecovery,
  onRotateKey,
}: RailProps) {
  const tr = useTr();
  const locale = useLocale();
  return (
    <aside className="tv-sync-rail">
      <div className="tv-sync-rail-label">{tr('Where it syncs', '同步方式')}</div>
      <ul className="tv-sync-server-list">
        <li className="tv-sync-server-row">
          <span aria-hidden="true" className="tv-sync-server-mark" />
          <span>{tr('Your own server', '你自己的服务器')}</span>
          <span className="tv-sync-server-url" title={status.serverUrl ?? ''}>
            {status.serverUrl ?? tr('not set', '未设置')}
          </span>
        </li>
      </ul>
      <p className="tv-sync-rail-note">
        {tr(
          'Self-hosted is the only option in this version. The server stores ciphertext and nothing else — it never holds a key, so it can never read a snippet.',
          '此版本仅支持自建服务器。服务器只存储密文，别无其他 —— 它从不持有密钥，因此永远无法读取片段。',
        )}
      </p>

      <div className="tv-sync-rail-label">{tr('Pair a device', '配对设备')}</div>
      <p className="tv-sync-rail-note">
        {tr(
          'Pairing exchanges keys directly between the two devices. The server only relays them, and both screens show the same four groups of characters so a relay cannot sit in between.',
          '配对时密钥在两台设备之间直接交换。服务器只做转发，两台设备的屏幕会显示相同的四组字符，因此转发者无法插入其中。',
        )}
      </p>
      <div className="tv-sync-actions">
        <button type="button" className="tv-sync-button" onClick={onPairDevice}>
          {tr('Show a pairing code', '显示配对码')}
        </button>
      </div>

      <div className="tv-sync-rail-foot">
        <p className="tv-sync-rail-note">
          {status.recoveryExportedAt === null
            ? tr('No recovery code exported from this device.', '这台设备尚未导出恢复码。')
            : tr(
                `Recovery code last exported ${ageLabel(status.recoveryExportedAt, now, locale)}.`,
                `恢复码上次导出于 ${ageLabel(status.recoveryExportedAt, now, locale)}。`,
              )}
        </p>
        <div className="tv-sync-actions">
          <button
            type="button"
            className="tv-sync-button"
            disabled={busy || !status.vaultReady}
            onClick={onExportRecovery}
          >
            {status.recoveryExportedAt === null
              ? tr('Export a recovery code', '导出恢复码')
              : tr('Export again', '再次导出')}
          </button>
          <button type="button" className="tv-sync-button" disabled={busy} onClick={onRotateKey}>
            {tr('Rotate the key', '轮换密钥')}
          </button>
        </div>
        {!status.vaultReady && (
          <p className="tv-sync-rail-note">
            {tr(
              'A recovery code protects the vault, so it needs a vault to exist first. Your ordinary snippets sync without one.',
              '恢复码用于保护保险库，因此需要先有保险库。你的普通片段无需恢复码即可同步。',
            )}
          </p>
        )}
      </div>
    </aside>
  );
}

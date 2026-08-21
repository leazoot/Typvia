// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Sync — status first, contextual "Sync now" only when there is something to
 * send, server URL folded until clicked, the device as a name rather than a
 * debug line; pairing/conflicts stay on the Sync page, reached from here.
 */
import { useLocale, useTr } from '@typvia/ui';
import { relativeTime, useSyncSettings } from '@typvia/ui/sync';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { PrefSection } from './pref-section';
import { Chip, Fold, FoldRow, KV, SetLabel, SetRow, StatusWord } from './pref-kit';

export function SyncPref() {
  const tr = useTr();
  const locale = useLocale();
  const navigate = useNavigate();
  const { status, busy, notice, toggle, runNow } = useSyncSettings();
  const [urlOpen, setUrlOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const copyUrl = async () => {
    if (!status?.serverUrl) return;
    try {
      await navigator.clipboard.writeText(status.serverUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // URL stays revealed in the fold regardless.
    }
  };

  const pending = status?.pendingBacklog ?? 0;
  const conflicts = status?.conflictCount ?? 0;

  const sub = !status
    ? '…'
    : !status.available
      ? tr('No secure key storage on this device', '这台设备没有安全密钥存储')
      : !status.configured
        ? tr('Keep devices in sync, end-to-end encrypted', '在设备间同步,端到端加密')
        : !status.enabled
          ? tr('Paused — changes stay on this Mac', '已暂停——更改留在本机')
          : conflicts > 0
            ? tr(
                `${String(conflicts)} conflict${conflicts === 1 ? '' : 's'} need a decision`,
                `${String(conflicts)} 个冲突待处理`,
              )
            : pending > 0
              ? tr(
                  `${String(pending)} change${pending === 1 ? '' : 's'} waiting to send`,
                  `${String(pending)} 项更改等待发送`,
                )
              : status.lastSyncAt
                ? tr(
                    `Synced ${relativeTime(status.lastSyncAt, Date.now(), locale)}`,
                    `${relativeTime(status.lastSyncAt, Date.now(), locale)}已同步`,
                  )
                : tr('Up to date', '已是最新');

  return (
    <PrefSection
      id="sync"
      glyph="sync"
      name={tr('Sync', '同步')}
      sub={sub}
      status={
        !status || !status.configured ? undefined : !status.enabled ? (
          <StatusWord kind="idle">{tr('Paused', '已暂停')}</StatusWord>
        ) : conflicts > 0 ? (
          <StatusWord kind="warn">{tr('Needs attention', '需要处理')}</StatusWord>
        ) : pending > 0 ? (
          <StatusWord kind="ok">{tr('On', '开启')}</StatusWord>
        ) : (
          <span className="tvp-status-inline">✓</span>
        )
      }
      actions={
        status?.enabled && pending > 0 ? (
          <Chip cta disabled={busy} onClick={() => void runNow()}>
            {tr('Sync now', '立即同步')}
          </Chip>
        ) : status?.enabled ? (
          <Chip disabled={busy} onClick={() => void runNow()}>
            {tr('Sync now', '立即同步')}
          </Chip>
        ) : undefined
      }
    >
      {!status ? null : !status.available ? (
        <SetRow
          title={tr('Sync can’t be set up here', '无法在此设置同步')}
          description={tr(
            'This device has no secure key storage. Everything else works normally.',
            '这台设备没有安全密钥存储。其余功能一切照常。',
          )}
        />
      ) : !status.configured ? (
        <SetRow
          title={tr('Not set up', '尚未设置')}
          description={tr(
            'Create an account, join another device, or restore with your recovery code.',
            '新建账户、加入其他设备,或用恢复码取回。',
          )}
          actions={
            <button type="button" className="tvp-btn" onClick={() => void navigate('/sync')}>
              {tr('Set up sync', '设置同步')}
            </button>
          }
        />
      ) : (
        <>
          <SetLabel>{tr('Status', '状态')}</SetLabel>
          <SetRow
            title={status.enabled ? tr('Sync on', '同步开启') : tr('Sync paused', '同步已暂停')}
            description={
              notice ??
              (status.enabled
                ? tr('The server only ever sees ciphertext.', '服务器只见密文。')
                : tr('Changes stay on this Mac until you resume.', '恢复前更改留在本机。'))
            }
            actions={
              <Chip disabled={busy} onClick={() => void toggle()}>
                {status.enabled ? tr('Pause', '暂停') : tr('Resume', '恢复')}
              </Chip>
            }
          />
          {conflicts > 0 && (
            <SetRow
              title={tr(
                `${String(conflicts)} conflict${conflicts === 1 ? '' : 's'}`,
                `${String(conflicts)} 个冲突`,
              )}
              description={tr(
                'Both versions are kept until you decide.',
                '两个版本都保留,等你裁定。',
              )}
              actions={
                <Chip cta onClick={() => void navigate('/sync')}>
                  {tr('Resolve', '去处理')}
                </Chip>
              }
            />
          )}

          <SetLabel>{tr('Server', '服务器')}</SetLabel>
          <SetRow
            onClick={() => setUrlOpen((current) => !current)}
            title={
              status.serverUrl?.includes('127.0.0.1') || status.serverUrl?.includes('localhost')
                ? tr('Local server', '本地服务器')
                : tr('Your server', '自有服务器')
            }
            actions={
              <Chip onClick={() => void copyUrl()}>
                {copied ? tr('Copied', '已复制') : tr('Copy', '拷贝')}
              </Chip>
            }
          />
          <Fold open={urlOpen}>
            <span className="tvp-mono">{status.serverUrl ?? '—'}</span>
          </Fold>

          <SetLabel>{tr('Devices', '设备')}</SetLabel>
          <SetRow
            title={tr('This Mac', '这台 Mac')}
            description={status.deviceName}
            status={<StatusWord kind="ok">{tr('Trusted', '受信任')}</StatusWord>}
          />
          <SetRow
            title={tr('Paired devices & recovery', '配对设备与恢复')}
            description={tr(
              'Pairing, revocation and the recovery code live on the Sync page.',
              '配对、撤销与恢复码在同步页管理。',
            )}
            actions={<Chip onClick={() => void navigate('/sync')}>{tr('Open', '打开')}</Chip>}
          />

          <FoldRow
            label={tr('Advanced', '高级')}
            open={advancedOpen}
            onToggle={() => setAdvancedOpen((current) => !current)}
          />
          <Fold open={advancedOpen}>
            <KV
              rows={[
                [tr('Device ID', '设备 ID'), status.deviceId],
                [tr('Account', '账户'), status.accountId ?? '—'],
                [tr('Key generation', '密钥代数'), String(status.keyGeneration)],
              ]}
            />
          </Fold>
        </>
      )}
    </PrefSection>
  );
}

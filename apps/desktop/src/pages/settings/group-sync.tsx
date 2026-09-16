// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { syncDevices, type SyncDevice } from '@typvia/shared';
import { counted, useTr, type Tr } from '@typvia/ui';
import { useSyncSettings } from '@typvia/ui/sync';
import { useEffect, useId, useState } from 'react';
import { useNavigate } from 'react-router';
import { GroupTitle, TextAction } from '../../paper/kit';
import { Said, SettingRow, State } from './settings-kit';

export function SyncGroup() {
  const tr = useTr();
  const navigate = useNavigate();
  const { status, busy, notice, toggle, runNow } = useSyncSettings();
  const [devices, setDevices] = useState<SyncDevice[] | null>(null);
  const configured = status?.configured === true;

  useEffect(() => {
    if (!configured) return;
    syncDevices()
      .then((rows) => setDevices(rows.filter((row) => row.revokedAt === null)))
      .catch(() => setDevices(null));
  }, [configured]);

  if (status === null) return null;

  if (!status.available || !status.configured) {
    return (
      <section className="tvs-group">
        <GroupTitle>{tr('Sync', '同步')}</GroupTitle>
        <SettingRow label={tr('Sync', '同步')}>
          <div className="tvs-inline">
            <State kind="idle">
              {!status.available
                ? tr('Not available on this device', '这台设备用不了')
                : tr('Not set up', '还没设置')}
            </State>
            {status.available && (
              <TextAction primary onClick={() => void navigate('/sync')}>
                {tr('Set up sync', '设置同步')}
              </TextAction>
            )}
          </div>
        </SettingRow>
      </section>
    );
  }

  const targets =
    devices === null || devices.length === 0
      ? tr('This device', '当前')
      : devices.map((device) => platformName(device, tr)).join(tr(', ', '、'));
  const nodes = devices ?? [];
  const count = Math.max(nodes.length, 1);
  const pending = status.pendingBacklog;
  const conflicts = status.conflictCount;

  return (
    <section className="tvs-group">
      <GroupTitle
        trailing={
          <button type="button" className="tvs-group-action" onClick={() => void navigate('/sync')}>
            {tr('Devices & recovery', '设备与恢复')}
          </button>
        }
      >
        {tr('Sync', '同步')}
      </GroupTitle>
      <SettingRow label={counted(tr, count, 'device', 'devices', `${String(count)} 台设备`)}>
        <DeviceSpine devices={nodes} flowing={status.enabled} />
        <div className="tvs-inline">
          <span className="tvs-text is-meta">
            {!status.enabled
              ? tr('Paused', '已暂停')
              : pending > 0 || status.lastSyncAt === null
                ? tr('Not synced', '未同步')
                : tr('Synced', '已同步')}
          </span>
          {status.enabled && (
            <TextAction disabled={busy} onClick={() => void runNow()}>
              {tr('Sync once now', '现在同步一次')}
            </TextAction>
          )}
          <TextAction disabled={busy} onClick={() => void toggle()}>
            {status.enabled ? tr('Pause', '暂停') : tr('Resume sync', '继续同步')}
          </TextAction>
        </div>
        {conflicts > 0 && (
          <div className="tvs-inline">
            <span className="tvs-text is-value">
              {tr(
                `${String(conflicts)} conflict${conflicts === 1 ? '' : 's'} wait for you`,
                `${String(conflicts)} 个冲突等你裁定`,
              )}
            </span>
            <TextAction primary onClick={() => void navigate('/sync/conflicts')}>
              {tr('Resolve', '去处理')}
            </TextAction>
          </div>
        )}
        {notice !== null && <Said>{notice}</Said>}
      </SettingRow>
      <SettingRow label={tr('Syncs to', '同步到')}>
        <p className="tvs-text is-value">{targets}</p>
      </SettingRow>
    </section>
  );
}

/** A device named by what it is; this one is simply the current device. */
function platformName(device: SyncDevice, tr: Tr): string {
  if (device.isThisDevice) return tr('This device', '当前');
  switch (device.platform) {
    case 'ios':
      return 'iPhone';
    case 'android':
      return 'Android';
    case 'windows':
      return 'Windows';
    case 'macos':
      return 'Mac';
    default:
      return device.name;
  }
}

const SPINE_WIDTH = 600;
const SPINE_EDGE = 20;

/**
 * The devices as nodes on one flowing line: solid where sync is running,
 * dotted 2/9 where it waits. This device is the large node.
 */
function DeviceSpine({ devices, flowing }: { devices: SyncDevice[]; flowing: boolean }) {
  const tr = useTr();
  const gradient = useId();
  const alone = devices.length <= 1;
  const shown: { key: string; label: string; self: boolean }[] =
    devices.length === 0
      ? [{ key: 'self', label: tr('This Mac', '这台 Mac'), self: true }]
      : devices.map((device) => ({
          key: device.deviceId,
          label: device.isThisDevice ? tr('This Mac', '这台 Mac') : device.name,
          self: device.isThisDevice,
        }));

  // One node sits centre stage with a dotted tail towards devices still to come.
  const span = SPINE_WIDTH - SPINE_EDGE * 2;
  const points = shown.map((node, index) => ({
    ...node,
    x: alone ? 240 : SPINE_EDGE + (span * index) / (shown.length - 1),
    y: index % 2 === 0 ? 60 : 52,
  }));

  const segments = points.slice(1).map((to, index) => {
    const from = points[index] ?? to;
    const dx = to.x - from.x;
    const bend = index % 2 === 0 ? Math.min(from.y, to.y) - 36 : Math.max(from.y, to.y) + 32;
    return `M${from.x} ${from.y}C${from.x + dx * 0.3} ${bend} ${from.x + dx * 0.7} ${bend} ${to.x} ${to.y}`;
  });

  return (
    <svg
      className="tvs-spine"
      viewBox={`0 0 ${SPINE_WIDTH} 120`}
      role="img"
      aria-label={shown.map((node) => node.label).join(' · ')}
    >
      <defs>
        <linearGradient id={gradient} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" className="tvs-spine-stop-a" />
          <stop offset="1" className="tvs-spine-stop-b" />
        </linearGradient>
      </defs>
      {segments.map((path) =>
        flowing ? (
          <path key={path} d={path} stroke={`url(#${gradient})`} className="tvs-spine-path" />
        ) : (
          <path key={path} d={path} className="tvs-spine-path is-waiting" />
        ),
      )}
      {alone && (
        <>
          <path d="M240 60C330 86 420 70 580 42" className="tvs-spine-path is-waiting" />
          <circle cx="580" cy="42" r="3.5" className="tvs-spine-end" />
          <text x="580" y="70" textAnchor="end" className="tvs-spine-label is-faint">
            {tr('No other device yet', '还没有别的设备')}
          </text>
        </>
      )}
      {points.map((node) =>
        node.self ? (
          <g key={node.key}>
            <circle cx={node.x} cy={node.y} r="12" className="tvs-spine-halo" />
            <circle cx={node.x} cy={node.y} r="7" className="tvs-spine-self" />
            <text x={node.x} y={node.y + 28} textAnchor="middle" className="tvs-spine-label">
              {node.label}
            </text>
          </g>
        ) : (
          <g key={node.key}>
            <circle cx={node.x} cy={node.y} r="4" className="tvs-spine-node" />
            <text
              x={node.x}
              y={node.y + 28}
              textAnchor={
                node.x <= SPINE_EDGE
                  ? 'start'
                  : node.x >= SPINE_WIDTH - SPINE_EDGE
                    ? 'end'
                    : 'middle'
              }
              className="tvs-spine-label"
            >
              {node.label}
            </text>
          </g>
        ),
      )}
    </svg>
  );
}

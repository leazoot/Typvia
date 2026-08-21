// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * First-use state for Sync (17-state system: first-use). Three real choices,
 * stated plainly — start an account here, join one from another device, or
 * recover one when every device is gone. No illustration, no marketing.
 */
import type { SyncStatus } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useSyncSetup } from '@typvia/ui/sync';
import { useNavigate } from 'react-router';

interface SetupProps {
  status: SyncStatus;
  onDone: () => void;
}

export function SyncSetup({ status, onDone }: SetupProps) {
  const tr = useTr();
  const navigate = useNavigate();
  const setup = useSyncSetup(onDone);
  const { serverUrl, busy, error } = setup;

  if (!status.available) {
    return (
      <main className="tv-sync-main">
        <h1 className="tv-sync-title">
          {tr('Sync needs secure key storage.', '同步需要安全的密钥存储。')}
        </h1>
        <p className="tv-sync-subtitle">
          {tr(
            'This device cannot store keys securely, so sync stays off for now',
            '此设备无法安全保存密钥，同步暂不可用',
          )}
        </p>
        <p className="tv-sync-lead">
          {tr(
            'Everything else works exactly as before — snippets, the panel, templates and the vault are all local. Sync is the one feature that needs the system keychain, and this device did not make it available.',
            '其他一切照常工作 —— 片段、面板、模板和保险库都在本地。同步是唯一需要系统钥匙串的功能，而这台设备没有提供它。',
          )}
        </p>
      </main>
    );
  }

  return (
    <main className="tv-sync-main">
      <h1 className="tv-sync-title">
        {tr('Your text lives on this device only.', '你的文本只保存在这台设备上。')}
      </h1>
      <p className="tv-sync-subtitle">
        {tr(
          'Sync is optional · the server stores only ciphertext · turn it off any time',
          '同步是可选的 · 服务器只存密文 · 随时可以关闭',
        )}
      </p>

      <div className="tv-sync-empty">
        <div className="tv-sync-rail-label">{tr('Start an account', '新建账户')}</div>
        <div
          className="tv-sync-actions"
          role="group"
          aria-label={tr('Where the ciphertext lives', '密文存放位置')}
        >
          <button
            type="button"
            className="tv-sync-button"
            aria-pressed={setup.kind === 'server'}
            onClick={() => setup.setKind('server')}
          >
            {tr('My own server', '自建服务器')}
          </button>
          <button
            type="button"
            className="tv-sync-button"
            aria-pressed={setup.kind === 'webdav'}
            onClick={() => setup.setKind('webdav')}
          >
            {tr('My WebDAV storage', '我的 WebDAV 网盘')}
          </button>
        </div>
        <p className="tv-sync-lead">
          {setup.kind === 'webdav'
            ? tr(
                'Point Typvia at a WebDAV folder you own — Nextcloud, or any drive that speaks WebDAV. It holds ciphertext and nothing else; no key ever leaves this device.',
                '将 Typvia 指向你自己的 WebDAV 目录 —— Nextcloud 或任何支持 WebDAV 的网盘。那里只存放密文，密钥从不离开这台设备。',
              )
            : tr(
                'Point Typvia at a server you run. It stores ciphertext and nothing else — no key ever leaves this device, so the server can never read a snippet.',
                '将 Typvia 指向你自己运行的服务器。服务器只存储密文，别无其他 —— 密钥从不离开这台设备，因此服务器永远无法读取片段。',
              )}
        </p>
        <label className="tv-sync-rail-note" htmlFor="tv-sync-server">
          {setup.kind === 'webdav'
            ? tr('WebDAV address', 'WebDAV 地址')
            : tr('Server address', '服务器地址')}
        </label>
        <input
          id="tv-sync-server"
          className="tv-sync-field"
          type="url"
          inputMode="url"
          spellCheck={false}
          placeholder={
            setup.kind === 'webdav' ? 'https://dav.example.com/typvia' : 'https://sync.example.com'
          }
          value={serverUrl}
          onChange={(event) => {
            setup.setServerUrl(event.target.value);
          }}
        />
        {setup.kind === 'webdav' && (
          <>
            <label className="tv-sync-rail-note" htmlFor="tv-sync-dav-user">
              {tr('Username · password', '用户名 · 密码')}
            </label>
            <div className="tv-sync-actions">
              <input
                id="tv-sync-dav-user"
                className="tv-sync-field"
                type="text"
                autoComplete="off"
                spellCheck={false}
                placeholder={tr('Username', '用户名')}
                value={setup.username}
                onChange={(event) => {
                  setup.setUsername(event.target.value);
                }}
              />
              <input
                className="tv-sync-field"
                type="password"
                autoComplete="off"
                aria-label={tr('WebDAV password', 'WebDAV 密码')}
                placeholder={tr('Password', '密码')}
                value={setup.password}
                onChange={(event) => {
                  setup.setPassword(event.target.value);
                }}
              />
            </div>
            <p className="tv-sync-rail-note">
              {tr(
                'Stored in the system keychain, never in the library.',
                '保存在系统钥匙串中，不入库。',
              )}
            </p>
          </>
        )}
        <div className="tv-sync-actions tv-sync-run-row">
          <button
            type="button"
            className="tv-sync-primary"
            disabled={busy || !setup.ready}
            onClick={() => void setup.start()}
          >
            {tr('Start an account here', '在这里新建账户')}
          </button>
          {error !== null && (
            <span className="tv-sync-row-meta" role="status">
              {error} {tr('Nothing was changed on this device.', '这台设备上没有任何改动。')}
            </span>
          )}
        </div>

        <div className="tv-sync-rail-label tv-sync-run-row">
          {tr('Already have one', '已有账户')}
        </div>
        <p className="tv-sync-lead">
          {tr(
            'Join from a device that already holds the library, or recover the account with the recovery code if none of your devices are left.',
            '从已持有片段库的设备加入；如果所有设备都已丢失，则用恢复码恢复账户。',
          )}
        </p>
        <div className="tv-sync-actions">
          <button
            type="button"
            className="tv-sync-button"
            onClick={() => void navigate('/sync/pair')}
          >
            {tr('Join from another device', '从另一台设备加入')}
          </button>
          <button
            type="button"
            className="tv-sync-button"
            onClick={() => void navigate('/sync/recover')}
          >
            {tr('Recover with a recovery code', '用恢复码恢复')}
          </button>
        </div>
      </div>
    </main>
  );
}

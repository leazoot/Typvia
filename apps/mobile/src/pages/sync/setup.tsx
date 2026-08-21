// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * First-use state for sync on a phone (17-state system: first-use). Three
 * real choices, stated plainly — start an account here, join one from another
 * device, or recover one when every device is gone.
 */
import type { SyncStatus } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useSyncSetup } from '@typvia/ui/sync';
import { SettingsScreen } from '../settings/screen';
import './sync.css';

interface SetupProps {
  status: SyncStatus;
  onBack: () => void;
  onDone: () => void;
  onJoin: () => void;
  onRecover: () => void;
}

export function SyncSetup({ status, onBack, onDone, onJoin, onRecover }: SetupProps) {
  const tr = useTr();
  const setup = useSyncSetup(onDone);
  const { serverUrl, busy, error } = setup;

  if (!status.available) {
    return (
      <SettingsScreen
        parent={tr('Settings', '设置')}
        title={tr('Sync needs secure key storage.', '同步需要安全的密钥存储。')}
        onBack={onBack}
      >
        <p className="tv-sync-lead">
          {tr(
            'Everything else works exactly as before — snippets, search and the vault are all local. Sync is the one feature that needs the system keystore, and this device did not make it available.',
            '其他一切照常工作 —— 片段、搜索和保险库都在本地。同步是唯一需要系统密钥库的功能,而这台设备没有提供它。',
          )}
        </p>
      </SettingsScreen>
    );
  }

  return (
    <SettingsScreen
      parent={tr('Settings', '设置')}
      title={tr('Your text lives on this device only.', '你的文本只存在这台设备上。')}
      onBack={onBack}
    >
      <div className="tv-sync-rail-label">{tr('Start an account', '新建账户')}</div>
      <div
        className="tv-msync-kind-row"
        role="group"
        aria-label={tr('Where the ciphertext lives', '密文存放位置')}
      >
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={setup.kind === 'server'}
          onClick={() => setup.setKind('server')}
        >
          {tr('My own server', '自建服务器')}
        </button>
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={setup.kind === 'webdav'}
          onClick={() => setup.setKind('webdav')}
        >
          {tr('My WebDAV storage', '我的 WebDAV 网盘')}
        </button>
      </div>
      <p className="tv-sync-lead">
        {setup.kind === 'webdav'
          ? tr(
              'Point Typvia at a WebDAV folder you own. It holds ciphertext and nothing else; no key ever leaves this device.',
              '把 Typvia 指向你自己的 WebDAV 目录。那里只存放密文,密钥从不离开这台设备。',
            )
          : tr(
              'Point Typvia at a server you run. It stores ciphertext and nothing else — no key ever leaves this device, so the server can never read a snippet.',
              '把 Typvia 指向你自己运行的服务器。它只存密文,别无其他 —— 密钥从不离开这台设备,所以服务器永远读不到任何片段。',
            )}
      </p>
      <label className="tv-mset-field-label" htmlFor="tv-msync-server">
        {setup.kind === 'webdav'
          ? tr('WebDAV address', 'WebDAV 地址')
          : tr('Server address', '服务器地址')}
      </label>
      <input
        id="tv-msync-server"
        className="tv-mset-field"
        type="url"
        inputMode="url"
        autoCapitalize="none"
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
          <label className="tv-mset-field-label" htmlFor="tv-msync-dav-user">
            {tr('Username', '用户名')}
          </label>
          <input
            id="tv-msync-dav-user"
            className="tv-mset-field"
            type="text"
            autoCapitalize="none"
            autoComplete="off"
            spellCheck={false}
            value={setup.username}
            onChange={(event) => {
              setup.setUsername(event.target.value);
            }}
          />
          <label className="tv-mset-field-label" htmlFor="tv-msync-dav-pass">
            {tr('Password', '密码')}
          </label>
          <input
            id="tv-msync-dav-pass"
            className="tv-mset-field"
            type="password"
            autoComplete="off"
            value={setup.password}
            onChange={(event) => {
              setup.setPassword(event.target.value);
            }}
          />
          <p className="tv-mset-field-label">
            {tr(
              'Stored in the system keychain, never in the library.',
              '保存在系统钥匙串中,不入库。',
            )}
          </p>
        </>
      )}
      <button
        type="button"
        className="tv-mset-button is-primary"
        disabled={busy || !setup.ready}
        onClick={() => void setup.start()}
      >
        {tr('Start an account here', '在这里新建账户')}
      </button>
      {error !== null && (
        <span className="tv-msync-notice" role="status">
          {error} {tr('Nothing was changed on this device.', '本机没有任何改动。')}
        </span>
      )}

      <div className="tv-sync-rail-label">{tr('Already have one', '已有账户')}</div>
      <p className="tv-sync-lead">
        {tr(
          'Join from a device that already holds the library, or recover the account with the recovery code if none of your devices are left.',
          '从已经保存了片段库的设备加入;若你的设备一台都不剩,则用恢复码恢复账户。',
        )}
      </p>
      <button type="button" className="tv-mset-button" onClick={onJoin}>
        {tr('Join from another device', '从另一台设备加入')}
      </button>
      <button type="button" className="tv-mset-button" onClick={onRecover}>
        {tr('Recover with a recovery code', '用恢复码恢复')}
      </button>
    </SettingsScreen>
  );
}

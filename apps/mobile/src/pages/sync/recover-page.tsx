// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Account recovery on a phone. Every paired device is gone; this
 * one takes the account over with the recovery code. The consequence is
 * stated before the button because it is permanent: every previous device is
 * revoked and must pair again.
 */
import { useTr } from '@typvia/ui';
import { useAccountRecover } from '@typvia/ui/sync';
import { SettingsScreen } from '../settings/screen';
import './sync.css';

interface RecoverPageProps {
  onBack: () => void;
  onExportRecovery: () => void;
}

export function MobileRecoverPage({ onBack, onExportRecovery }: RecoverPageProps) {
  const tr = useTr();
  const flow = useAccountRecover();
  const { serverUrl, accountId, code, masterPassword, busy, error, done } = flow;

  if (done) {
    return (
      <SettingsScreen
        parent={tr('Sync', '同步')}
        title={tr('This device is the account now.', '这台设备现在就是账户。')}
        onBack={onBack}
      >
        <p className="tv-sync-lead">
          {tr(
            'Your snippets are arriving. Every device that was paired before has been revoked and will have to pair again from scratch.',
            '你的片段正在陆续到达。之前配对过的每一台设备都已被吊销,必须重新从头配对。',
          )}
        </p>
        <p className="tv-sync-lead">
          {tr(
            'The recovery code you just used is spent as an account anchor. Export a fresh one now, so there is still a way back if this device is lost too.',
            '你刚使用的恢复码作为账户锚点已经用掉了。现在就导出一个新的,这样即使这台设备也丢失,仍有一条回来的路。',
          )}
        </p>
        <button type="button" className="tv-mset-button is-primary" onClick={onExportRecovery}>
          {tr('Export a new recovery code', '导出新的恢复码')}
        </button>
        <button type="button" className="tv-mset-button" onClick={onBack}>
          {tr('Later', '稍后')}
        </button>
      </SettingsScreen>
    );
  }

  return (
    <SettingsScreen
      parent={tr('Sync', '同步')}
      title={tr('Recover an account.', '恢复账户。')}
      intro={tr(
        "Use this when none of your paired devices are left. Recovering makes this device the account's anchor and revokes every previous one — if another device still works, pair from it instead.",
        '当已配对的设备一台都不剩时才使用。恢复会让这台设备成为账户的锚点,并吊销之前的每一台设备 —— 如果还有设备能用,请改用它来配对。',
      )}
      onBack={onBack}
    >
      <div
        className="tv-msync-kind-row"
        role="group"
        aria-label={tr('Where the ciphertext lives', '密文存放位置')}
      >
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={flow.kind === 'server'}
          onClick={() => flow.setKind('server')}
        >
          {tr('My own server', '自建服务器')}
        </button>
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={flow.kind === 'webdav'}
          onClick={() => flow.setKind('webdav')}
        >
          {tr('My WebDAV storage', '我的 WebDAV 网盘')}
        </button>
      </div>
      <label className="tv-mset-field-label" htmlFor="tv-msync-recover-server">
        {flow.kind === 'webdav'
          ? tr('WebDAV address', 'WebDAV 地址')
          : tr('Server address', '服务器地址')}
      </label>
      <input
        id="tv-msync-recover-server"
        className="tv-mset-field"
        type="url"
        inputMode="url"
        autoCapitalize="none"
        spellCheck={false}
        placeholder={
          flow.kind === 'webdav' ? 'https://dav.example.com/typvia' : 'https://sync.example.com'
        }
        value={serverUrl}
        onChange={(event) => {
          flow.setServerUrl(event.target.value);
        }}
      />
      {flow.kind === 'webdav' ? (
        <>
          <label className="tv-mset-field-label" htmlFor="tv-msync-recover-dav-user">
            {tr('Username', '用户名')}
          </label>
          <input
            id="tv-msync-recover-dav-user"
            className="tv-mset-field"
            type="text"
            autoCapitalize="none"
            autoComplete="off"
            spellCheck={false}
            value={flow.username}
            onChange={(event) => {
              flow.setUsername(event.target.value);
            }}
          />
          <label className="tv-mset-field-label" htmlFor="tv-msync-recover-dav-pass">
            {tr('Password', '密码')}
          </label>
          <input
            id="tv-msync-recover-dav-pass"
            className="tv-mset-field"
            type="password"
            autoComplete="off"
            value={flow.password}
            onChange={(event) => {
              flow.setPassword(event.target.value);
            }}
          />
        </>
      ) : (
        <>
          <label className="tv-mset-field-label" htmlFor="tv-msync-recover-account">
            {tr('Account id', '账户 ID')}
          </label>
          <input
            id="tv-msync-recover-account"
            className="tv-mset-field"
            autoCapitalize="none"
            spellCheck={false}
            value={accountId}
            onChange={(event) => {
              flow.setAccountId(event.target.value);
            }}
          />
        </>
      )}
      <label className="tv-mset-field-label" htmlFor="tv-msync-recover-code">
        {tr('Recovery code', '恢复码')}
      </label>
      <input
        id="tv-msync-recover-code"
        className="tv-mset-field"
        autoCapitalize="characters"
        autoComplete="off"
        spellCheck={false}
        placeholder="T1XX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
        value={code}
        onChange={(event) => {
          flow.setCode(event.target.value);
        }}
      />
      <label className="tv-mset-field-label" htmlFor="tv-msync-recover-password">
        {tr("A master password for this device's vault", '这台设备保险库的主密码')}
      </label>
      <input
        id="tv-msync-recover-password"
        className="tv-mset-field"
        type="password"
        autoComplete="new-password"
        value={masterPassword}
        onChange={(event) => {
          flow.setMasterPassword(event.target.value);
        }}
      />
      <button
        type="button"
        className="tv-mset-button is-primary"
        disabled={busy || !flow.ready}
        onClick={() => void flow.recover()}
      >
        {tr('Recover this account', '恢复这个账户')}
      </button>
      {error !== null && (
        <span className="tv-msync-notice" role="status">
          {error}
        </span>
      )}
    </SettingsScreen>
  );
}

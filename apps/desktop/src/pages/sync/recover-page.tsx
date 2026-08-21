// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Account recovery. Every paired device is
 * gone; this one takes the account over with the recovery code. The page
 * states the consequence before the button, because the consequence is
 * permanent: every previous device is revoked and must pair again.
 */
import { useTr } from '@typvia/ui';
import { useAccountRecover } from '@typvia/ui/sync';
import { useNavigate } from 'react-router';
import './sync.css';

export function RecoverPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const flow = useAccountRecover();
  const { serverUrl, accountId, code, masterPassword, busy, error, done } = flow;

  if (done) {
    return (
      <main className="tv-sync-main">
        <h1 className="tv-sync-title">
          {tr('This device is the account now.', '现在这台设备就是账户本身。')}
        </h1>
        <p className="tv-sync-subtitle">
          {tr(
            'Every old device is revoked · export a new recovery code',
            '旧设备已全部失效 · 请重新导出恢复码',
          )}
        </p>
        <div className="tv-sync-empty">
          <p className="tv-sync-lead">
            {tr(
              'Your snippets are arriving. Every device that was paired before has been revoked and will have to pair again from scratch.',
              '你的片段正在陆续到达。之前配对过的每台设备都已被撤销，需要重新配对。',
            )}
          </p>
          <p className="tv-sync-lead">
            {tr(
              'The recovery code you just used is spent as an account anchor. Export a fresh one now, so there is still a way back if this device is lost too.',
              '你刚使用的恢复码作为账户锚点已经耗尽。现在导出一个新的，这样即使这台设备也丢失，仍有一条回来的路。',
            )}
          </p>
          <div className="tv-sync-actions tv-sync-run-row">
            <button
              type="button"
              className="tv-sync-primary"
              onClick={() => void navigate('/sync/recovery')}
            >
              {tr('Export a new recovery code', '导出新的恢复码')}
            </button>
            <button type="button" className="tv-sync-link" onClick={() => void navigate('/sync')}>
              {tr('Later', '以后再说')}
            </button>
          </div>
        </div>
      </main>
    );
  }

  return (
    <main className="tv-sync-main">
      <h1 className="tv-sync-title">{tr('Recover an account.', '恢复账户。')}</h1>
      <p className="tv-sync-subtitle">
        {tr(
          'Take the account back on this device with the recovery code',
          '用恢复码在这台设备上取回账户',
        )}
      </p>
      <div className="tv-sync-empty">
        <p className="tv-sync-lead">
          {tr(
            "Use this when none of your paired devices are left. Recovering makes this device the account's anchor and revokes every previous one — if another device still works, pair from it instead.",
            '当已配对的设备一台都不剩时才使用此功能。恢复会让这台设备成为账户的锚点，并撤销之前的每一台设备 —— 如果还有设备能用，请改为从那台设备配对。',
          )}
        </p>

        <div
          className="tv-sync-actions"
          role="group"
          aria-label={tr('Where the ciphertext lives', '密文存放位置')}
        >
          <button
            type="button"
            className="tv-sync-button"
            aria-pressed={flow.kind === 'server'}
            onClick={() => flow.setKind('server')}
          >
            {tr('My own server', '自建服务器')}
          </button>
          <button
            type="button"
            className="tv-sync-button"
            aria-pressed={flow.kind === 'webdav'}
            onClick={() => flow.setKind('webdav')}
          >
            {tr('My WebDAV storage', '我的 WebDAV 网盘')}
          </button>
        </div>
        <label className="tv-sync-rail-note" htmlFor="tv-recover-server">
          {flow.kind === 'webdav'
            ? tr('WebDAV address', 'WebDAV 地址')
            : tr('Server address', '服务器地址')}
        </label>
        <input
          id="tv-recover-server"
          className="tv-sync-field"
          type="url"
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
            <label className="tv-sync-rail-note" htmlFor="tv-recover-dav-user">
              {tr('Username · password', '用户名 · 密码')}
            </label>
            <div className="tv-sync-actions">
              <input
                id="tv-recover-dav-user"
                className="tv-sync-field"
                type="text"
                autoComplete="off"
                spellCheck={false}
                placeholder={tr('Username', '用户名')}
                value={flow.username}
                onChange={(event) => {
                  flow.setUsername(event.target.value);
                }}
              />
              <input
                className="tv-sync-field"
                type="password"
                autoComplete="off"
                aria-label={tr('WebDAV password', 'WebDAV 密码')}
                placeholder={tr('Password', '密码')}
                value={flow.password}
                onChange={(event) => {
                  flow.setPassword(event.target.value);
                }}
              />
            </div>
          </>
        ) : (
          <>
            <label className="tv-sync-rail-note" htmlFor="tv-recover-account">
              {tr('Account id', '账户 ID')}
            </label>
            <input
              id="tv-recover-account"
              className="tv-sync-field"
              spellCheck={false}
              value={accountId}
              onChange={(event) => {
                flow.setAccountId(event.target.value);
              }}
            />
          </>
        )}
        <label className="tv-sync-rail-note" htmlFor="tv-recover-code">
          {tr('Recovery code', '恢复码')}
        </label>
        <input
          id="tv-recover-code"
          className="tv-sync-field"
          spellCheck={false}
          autoComplete="off"
          placeholder="T1XX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
          value={code}
          onChange={(event) => {
            flow.setCode(event.target.value);
          }}
        />
        <label className="tv-sync-rail-note" htmlFor="tv-recover-password">
          {tr("A master password for this device's vault", '这台设备保险库的主密码')}
        </label>
        <input
          id="tv-recover-password"
          className="tv-sync-field"
          type="password"
          autoComplete="new-password"
          value={masterPassword}
          onChange={(event) => {
            flow.setMasterPassword(event.target.value);
          }}
        />

        <div className="tv-sync-actions tv-sync-run-row">
          <button
            type="button"
            className="tv-sync-primary"
            disabled={busy || !flow.ready}
            onClick={() => void flow.recover()}
          >
            {tr('Recover this account', '恢复这个账户')}
          </button>
          <button type="button" className="tv-sync-link" onClick={() => void navigate('/sync')}>
            {tr('Back to devices', '返回设备列表')}
          </button>
        </div>
        {error !== null && (
          <p className="tv-sync-lead" role="status">
            {error}
          </p>
        )}
      </div>
    </main>
  );
}

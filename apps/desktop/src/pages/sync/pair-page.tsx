// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Pairing — a page, never a modal (the product has exactly one modal, the
 * Vault unlock). Both sides of the pairing exchange live here:
 *
 *  - "Show a code": this device is joining. It displays the code as a QR and
 *    as text, then waits for the other device's answer.
 *  - "Read a code": this device already holds the library. It reads the code
 *    and admits the other device.
 *
 * Both sides stop at the same gate: the four groups of characters must match
 * on both screens before anything is installed or admitted. That check is the
 * only thing standing between the user and a relaying server, so this screen
 * offers no way past it — and the check itself is the shared `SasCompare`,
 * which exists exactly once for both hosts.
 */
import { type SyncStatus, syncStatus } from '@typvia/shared';
import { QrCode, useTr } from '@typvia/ui';
import { SasCompare, usePairAdmit, usePairJoin } from '@typvia/ui/sync';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import './sync.css';

export function PairPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const [status, setStatus] = useState<SyncStatus | null>(null);

  useEffect(() => {
    void syncStatus()
      .then(setStatus)
      .catch(() => undefined);
  }, []);

  if (status === null) {
    return (
      <main className="tv-sync-main" aria-busy="true">
        <div className="tv-sync-skeleton-line" />
      </main>
    );
  }

  return (
    <main className="tv-sync-main">
      <h1 className="tv-sync-title">{tr('Pair a device.', '配对设备。')}</h1>
      <p className="tv-sync-subtitle">
        {tr(
          'Keys move directly between the two devices · the server only relays',
          '密钥在两台设备之间直接交换 · 服务器只做转发',
        )}
      </p>
      {status.configured ? (
        <AdmitSide onDone={() => void navigate('/sync')} vaultReady={status.vaultReady} />
      ) : (
        <JoinSide onDone={() => void navigate('/sync')} />
      )}
      <div className="tv-sync-actions tv-sync-run-row">
        <button type="button" className="tv-sync-link" onClick={() => void navigate('/sync')}>
          {tr('Back to devices', '返回设备列表')}
        </button>
      </div>
    </main>
  );
}

/**
 * This device already holds the library: read the other device's code, then
 * compare the four groups before admitting it.
 */
function AdmitSide({ onDone, vaultReady }: { onDone: () => void; vaultReady: boolean }) {
  const tr = useTr();
  const admit = usePairAdmit(vaultReady, onDone);
  const { code, check, allowVault, busy, error } = admit;

  if (check !== null) {
    return (
      <div className="tv-sync-empty">
        <SasCompare
          sas={check.sas}
          heading={tr(`Admit ${check.deviceName}?`, `允许 ${check.deviceName} 加入？`)}
          detail={`${check.deviceName} · ${check.platform}`}
        />
        {vaultReady && (
          <div className="tv-sync-run-row">
            <button
              type="button"
              className="tv-sync-button"
              aria-pressed={allowVault}
              onClick={() => {
                admit.setAllowVault(!allowVault);
              }}
            >
              {allowVault
                ? tr('Vault access: allowed', '保险库访问：允许')
                : tr('Vault access: withheld', '保险库访问：暂不开放')}
            </button>
            <p className="tv-sync-lead">
              {allowVault
                ? tr(
                    'This device will be able to open your secrets, using its own master password.',
                    '这台设备将能够用它自己的主密码打开你的机密内容。',
                  )
                : tr(
                    'This device will sync everything else and show your secrets as locked rows. It will never be able to open them until you grant access later.',
                    '这台设备会同步其余全部内容，机密内容显示为锁定行。在你以后授予访问权限之前，它始终无法打开它们。',
                  )}
            </p>
          </div>
        )}
        <div className="tv-sync-actions tv-sync-run-row">
          <button
            type="button"
            className="tv-sync-primary"
            disabled={busy}
            onClick={() => void admit.approve()}
          >
            {tr('The characters match — admit it', '字符一致 — 允许加入')}
          </button>
          <button type="button" className="tv-sync-button" onClick={admit.reject}>
            {tr('They do not match', '字符不一致')}
          </button>
        </div>
        {error !== null && (
          <p className="tv-sync-lead" role="status">
            {error}
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="tv-sync-empty">
      <div className="tv-sync-rail-label">{tr("Read the other device's code", '读取配对码')}</div>
      <p className="tv-sync-lead">
        {tr(
          'Open Typvia on the new device, choose "Join from another device", and paste the code it shows here.',
          '在新设备上打开 Typvia，选择「从另一台设备加入」，然后把它显示的配对码粘贴到这里。',
        )}
      </p>
      <textarea
        className="tv-sync-field"
        rows={4}
        spellCheck={false}
        aria-label={tr('Pairing code from the new device', '来自新设备的配对码')}
        placeholder="TYPVIA-PAIR.V1..."
        value={code}
        onChange={(event) => {
          admit.setCode(event.target.value);
        }}
      />
      <div className="tv-sync-actions tv-sync-run-row">
        <button
          type="button"
          className="tv-sync-primary"
          disabled={busy || code.trim() === ''}
          onClick={() => void admit.read()}
        >
          {tr('Read the code', '读取配对码')}
        </button>
        {error !== null && (
          <span className="tv-sync-row-meta" role="status">
            {error}
          </span>
        )}
      </div>
    </div>
  );
}

/**
 * This device is joining: show the code, wait for the offer, then compare the
 * four groups before installing anything.
 */
function JoinSide({ onDone }: { onDone: () => void }) {
  const tr = useTr();
  const join = usePairJoin(onDone);
  const { serverUrl, accountId, code, claim, masterPassword, busy, error } = join;

  if (claim !== null) {
    return (
      <div className="tv-sync-empty">
        <SasCompare
          sas={claim.sas}
          heading={tr('Do these match the other device?', '与另一台设备上显示的一致吗？')}
          detail={tr(
            `Account fingerprint ${claim.rootFingerprint}`,
            `账户指纹 ${claim.rootFingerprint}`,
          )}
        />
        <label className="tv-sync-rail-note" htmlFor="tv-pair-password">
          {tr(
            "Master password for this device's vault (only if the account has one)",
            '这台设备保险库的主密码（仅当账户设有保险库时需要）',
          )}
        </label>
        <input
          id="tv-pair-password"
          className="tv-sync-field"
          type="password"
          autoComplete="new-password"
          value={masterPassword}
          onChange={(event) => {
            join.setMasterPassword(event.target.value);
          }}
        />
        <div className="tv-sync-actions tv-sync-run-row">
          <button
            type="button"
            className="tv-sync-primary"
            disabled={busy}
            onClick={() => void join.finalize()}
          >
            {tr('The characters match — join', '字符一致 — 加入')}
          </button>
          <button type="button" className="tv-sync-button" onClick={join.stop}>
            {tr('They do not match — stop', '字符不一致 — 停止')}
          </button>
        </div>
        {error !== null && (
          <p className="tv-sync-lead" role="status">
            {error}{' '}
            {tr('Nothing has been installed on this device.', '这台设备上尚未安装任何内容。')}
          </p>
        )}
      </div>
    );
  }

  if (code !== null) {
    return (
      <div className="tv-sync-empty">
        <div className="tv-sync-rail-label">
          {tr('Show this to the other device', '向另一台设备展示此配对码')}
        </div>
        <div className="tv-sync-card">
          <QrCode value={code} label={tr('Pairing code', '配对码')} />
          <p className="tv-sync-lead">
            {tr(
              'On the device that already holds your library, open Sync & devices → Pair a device and scan this, or paste the text below.',
              '在已持有你片段库的设备上，打开「同步与设备 → 配对设备」，扫描此码，或粘贴下方文本。',
            )}
          </p>
          <textarea
            className="tv-sync-field"
            rows={4}
            readOnly
            aria-label={tr('Pairing code text', '配对码文本')}
            value={code}
          />
        </div>
        <p className="tv-sync-lead" role="status">
          {tr(
            'Waiting for the other device to confirm. Nothing has been installed yet.',
            '正在等待另一台设备确认。尚未安装任何内容。',
          )}
        </p>
      </div>
    );
  }

  return (
    <div className="tv-sync-empty">
      <div className="tv-sync-rail-label">{tr('Join an account', '加入账户')}</div>
      <div
        className="tv-sync-actions"
        role="group"
        aria-label={tr('Where the ciphertext lives', '密文存放位置')}
      >
        <button
          type="button"
          className="tv-sync-button"
          aria-pressed={join.kind === 'server'}
          onClick={() => join.setKind('server')}
        >
          {tr('My own server', '自建服务器')}
        </button>
        <button
          type="button"
          className="tv-sync-button"
          aria-pressed={join.kind === 'webdav'}
          onClick={() => join.setKind('webdav')}
        >
          {tr('My WebDAV storage', '我的 WebDAV 网盘')}
        </button>
      </div>
      <p className="tv-sync-lead">
        {join.kind === 'webdav'
          ? tr(
              'Point at the same WebDAV folder your other device uses — the account is read from the storage itself.',
              '指向另一台设备所用的同一个 WebDAV 目录 —— 账户信息会直接从存储中读取。',
            )
          : tr(
              'Read the server address and the account id from the device that already holds your library — they are both on its Sync & devices page.',
              '从已持有你片段库的设备上查看服务器地址与账户 ID —— 两者都在它的「同步与设备」页面上。',
            )}
      </p>
      <label className="tv-sync-rail-note" htmlFor="tv-pair-server">
        {join.kind === 'webdav'
          ? tr('WebDAV address', 'WebDAV 地址')
          : tr('Server address', '服务器地址')}
      </label>
      <input
        id="tv-pair-server"
        className="tv-sync-field"
        type="url"
        spellCheck={false}
        placeholder={
          join.kind === 'webdav' ? 'https://dav.example.com/typvia' : 'https://sync.example.com'
        }
        value={serverUrl}
        onChange={(event) => {
          join.setServerUrl(event.target.value);
        }}
      />
      {join.kind === 'webdav' ? (
        <>
          <label className="tv-sync-rail-note" htmlFor="tv-pair-dav-user">
            {tr('Username · password', '用户名 · 密码')}
          </label>
          <div className="tv-sync-actions">
            <input
              id="tv-pair-dav-user"
              className="tv-sync-field"
              type="text"
              autoComplete="off"
              spellCheck={false}
              placeholder={tr('Username', '用户名')}
              value={join.username}
              onChange={(event) => {
                join.setUsername(event.target.value);
              }}
            />
            <input
              className="tv-sync-field"
              type="password"
              autoComplete="off"
              aria-label={tr('WebDAV password', 'WebDAV 密码')}
              placeholder={tr('Password', '密码')}
              value={join.password}
              onChange={(event) => {
                join.setPassword(event.target.value);
              }}
            />
          </div>
        </>
      ) : (
        <>
          <label className="tv-sync-rail-note" htmlFor="tv-pair-account">
            {tr('Account id', '账户 ID')}
          </label>
          <input
            id="tv-pair-account"
            className="tv-sync-field"
            spellCheck={false}
            value={accountId}
            onChange={(event) => {
              join.setAccountId(event.target.value);
            }}
          />
        </>
      )}
      <div className="tv-sync-actions tv-sync-run-row">
        <button
          type="button"
          className="tv-sync-primary"
          disabled={busy || !join.ready}
          onClick={() => void join.begin()}
        >
          {tr('Show my pairing code', '显示我的配对码')}
        </button>
        {error !== null && (
          <span className="tv-sync-row-meta" role="status">
            {error}
          </span>
        )}
      </div>
    </div>
  );
}

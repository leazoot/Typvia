// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Pairing on a phone — a page, never a modal. Both sides of the exchange are
 * driven by the same hooks as the desktop page, so the gate cannot differ
 * between hosts: the four groups of characters must match on both screens
 * before
 * anything is installed or admitted, and there is no way past that here.
 *
 * Design deviation: no camera scan screen is drawn anywhere, and no camera
 * permission is requested. The admitting side therefore reads a pasted code
 * — the same path the desktop page offers — and the joining side shows the
 * QR the other device photographs plus the text to paste.
 */
import type { SyncStatus } from '@typvia/shared';
import { QrCode, useTr } from '@typvia/ui';
import { SasCompare, usePairAdmit, usePairJoin } from '@typvia/ui/sync';
import { SettingsScreen } from '../settings/screen';
import './sync.css';

interface PairPageProps {
  status: SyncStatus;
  onBack: () => void;
  onDone: () => void;
}

export function MobilePairPage({ status, onBack, onDone }: PairPageProps) {
  const tr = useTr();
  return (
    <SettingsScreen
      parent={tr('Sync', '同步')}
      title={tr('Pair a device.', '配对新设备。')}
      onBack={onBack}
    >
      {status.configured ? (
        <AdmitSide vaultReady={status.vaultReady} onDone={onDone} />
      ) : (
        <JoinSide onDone={onDone} />
      )}
    </SettingsScreen>
  );
}

/** This phone already holds the library: read the other device's code, then
 * compare the four groups before admitting it. */
function AdmitSide({ vaultReady, onDone }: { vaultReady: boolean; onDone: () => void }) {
  const tr = useTr();
  const admit = usePairAdmit(vaultReady, onDone);
  const { code, check, allowVault, busy, error } = admit;

  if (check !== null) {
    return (
      <>
        <SasCompare
          sas={check.sas}
          heading={tr(`Admit ${check.deviceName}?`, `允许 ${check.deviceName} 加入?`)}
          detail={`${check.deviceName} · ${check.platform}`}
        />
        {vaultReady && (
          <>
            <button
              type="button"
              className="tv-mset-button"
              aria-pressed={allowVault}
              onClick={() => {
                admit.setAllowVault(!allowVault);
              }}
            >
              {allowVault
                ? tr('Vault access: allowed', '保险库访问:允许')
                : tr('Vault access: withheld', '保险库访问:暂不开放')}
            </button>
            <p className="tv-sync-lead">
              {allowVault
                ? tr(
                    'That device will be able to open your secrets, using its own master password.',
                    '那台设备将能够用它自己的主密码打开你的秘密。',
                  )
                : tr(
                    'That device will sync everything else and show your secrets as locked rows. It will never be able to open them until you grant access later.',
                    '那台设备会同步其余所有内容,你的秘密只显示为锁定的条目。在你以后授权之前,它永远打不开它们。',
                  )}
            </p>
          </>
        )}
        <button
          type="button"
          className="tv-mset-button is-primary"
          disabled={busy}
          onClick={() => void admit.approve()}
        >
          {tr('The characters match — admit it', '字符一致 —— 允许加入')}
        </button>
        <button type="button" className="tv-mset-button" onClick={admit.reject}>
          {tr('They do not match', '不一致')}
        </button>
        {error !== null && (
          <span className="tv-msync-notice" role="status">
            {error}
          </span>
        )}
      </>
    );
  }

  return (
    <>
      <div className="tv-sync-rail-label">{tr("Read the other device's code", '读取配对码')}</div>
      <p className="tv-sync-lead">
        {tr(
          'Open Typvia on the new device, choose "Join from another device", and paste the code it shows here.',
          '在新设备上打开 Typvia,选择「从另一台设备加入」,把它显示的配对码粘贴到这里。',
        )}
      </p>
      <textarea
        className="tv-msync-code-text"
        rows={4}
        autoCapitalize="none"
        spellCheck={false}
        aria-label={tr('Pairing code from the new device', '来自新设备的配对码')}
        placeholder="TYPVIA-PAIR.V1..."
        value={code}
        onChange={(event) => {
          admit.setCode(event.target.value);
        }}
      />
      <button
        type="button"
        className="tv-mset-button is-primary"
        disabled={busy || code.trim() === ''}
        onClick={() => void admit.read()}
      >
        {tr('Read the code', '读取配对码')}
      </button>
      {error !== null && (
        <span className="tv-msync-notice" role="status">
          {error}
        </span>
      )}
    </>
  );
}

/** This phone is joining: show the code, wait for the offer, then compare the
 * four groups before installing anything. */
function JoinSide({ onDone }: { onDone: () => void }) {
  const tr = useTr();
  const join = usePairJoin(onDone);
  const { serverUrl, accountId, code, claim, masterPassword, busy, error } = join;

  if (claim !== null) {
    return (
      <>
        <SasCompare
          sas={claim.sas}
          heading={tr('Do these match the other device?', '这些字符与另一台设备一致吗?')}
          detail={tr(
            `Account fingerprint ${claim.rootFingerprint}`,
            `账户指纹 ${claim.rootFingerprint}`,
          )}
        />
        <label className="tv-mset-field-label" htmlFor="tv-msync-pair-password">
          {tr(
            "Master password for this device's vault (only if the account has one)",
            '这台设备保险库的主密码(仅当账户已设置保险库时需要)',
          )}
        </label>
        <input
          id="tv-msync-pair-password"
          className="tv-mset-field"
          type="password"
          autoComplete="new-password"
          value={masterPassword}
          onChange={(event) => {
            join.setMasterPassword(event.target.value);
          }}
        />
        <button
          type="button"
          className="tv-mset-button is-primary"
          disabled={busy}
          onClick={() => void join.finalize()}
        >
          {tr('The characters match — join', '字符一致 —— 加入')}
        </button>
        <button type="button" className="tv-mset-button" onClick={join.stop}>
          {tr('They do not match — stop', '不一致 —— 停止')}
        </button>
        {error !== null && (
          <span className="tv-msync-notice" role="status">
            {error}{' '}
            {tr('Nothing has been installed on this device.', '这台设备上没有安装任何内容。')}
          </span>
        )}
      </>
    );
  }

  if (code !== null) {
    return (
      <>
        <div className="tv-sync-rail-label">
          {tr('Show this to the other device', '展示配对码')}
        </div>
        <div className="tv-msync-code">
          <QrCode value={code} label={tr('Pairing code', '配对码')} size={196} />
          <textarea
            className="tv-msync-code-text"
            rows={4}
            readOnly
            aria-label={tr('Pairing code text', '配对码文本')}
            value={code}
          />
        </div>
        <p className="tv-sync-lead" role="status">
          {tr(
            'Waiting for the other device to confirm. Nothing has been installed yet.',
            '正在等待另一台设备确认。目前还没有安装任何内容。',
          )}
        </p>
      </>
    );
  }

  return (
    <>
      <div className="tv-sync-rail-label">{tr('Join an account', '加入账户')}</div>
      <p className="tv-sync-lead">
        {tr(
          'Read the server address and the account id from the device that already holds your library — they are both on its Sync screen.',
          '从已经保存你片段库的设备上读取服务器地址和账户 ID —— 两者都在它的同步页面上。',
        )}
      </p>
      <div
        className="tv-msync-kind-row"
        role="group"
        aria-label={tr('Where the ciphertext lives', '密文存放位置')}
      >
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={join.kind === 'server'}
          onClick={() => join.setKind('server')}
        >
          {tr('My own server', '自建服务器')}
        </button>
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={join.kind === 'webdav'}
          onClick={() => join.setKind('webdav')}
        >
          {tr('My WebDAV storage', '我的 WebDAV 网盘')}
        </button>
      </div>
      <label className="tv-mset-field-label" htmlFor="tv-msync-pair-server">
        {join.kind === 'webdav'
          ? tr('WebDAV address', 'WebDAV 地址')
          : tr('Server address', '服务器地址')}
      </label>
      <input
        id="tv-msync-pair-server"
        className="tv-mset-field"
        type="url"
        inputMode="url"
        autoCapitalize="none"
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
          <label className="tv-mset-field-label" htmlFor="tv-msync-pair-dav-user">
            {tr('Username', '用户名')}
          </label>
          <input
            id="tv-msync-pair-dav-user"
            className="tv-mset-field"
            type="text"
            autoCapitalize="none"
            autoComplete="off"
            spellCheck={false}
            value={join.username}
            onChange={(event) => {
              join.setUsername(event.target.value);
            }}
          />
          <label className="tv-mset-field-label" htmlFor="tv-msync-pair-dav-pass">
            {tr('Password', '密码')}
          </label>
          <input
            id="tv-msync-pair-dav-pass"
            className="tv-mset-field"
            type="password"
            autoComplete="off"
            value={join.password}
            onChange={(event) => {
              join.setPassword(event.target.value);
            }}
          />
        </>
      ) : (
        <>
          <label className="tv-mset-field-label" htmlFor="tv-msync-pair-account">
            {tr('Account id', '账户 ID')}
          </label>
          <input
            id="tv-msync-pair-account"
            className="tv-mset-field"
            autoCapitalize="none"
            spellCheck={false}
            value={accountId}
            onChange={(event) => {
              join.setAccountId(event.target.value);
            }}
          />
        </>
      )}
      <button
        type="button"
        className="tv-mset-button is-primary"
        disabled={busy || !join.ready}
        onClick={() => void join.begin()}
      >
        {tr('Show my pairing code', '显示我的配对码')}
      </button>
      {error !== null && (
        <span className="tv-msync-notice" role="status">
          {error}
        </span>
      )}
    </>
  );
}

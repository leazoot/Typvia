// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Recovery code. The code is shown exactly
 * once and never stored anywhere, so this page says so before it generates
 * one and refuses to pretend it can show it again.
 */
import { useTr } from '@typvia/ui';
import { useRecoveryExport } from '@typvia/ui/sync';
import { useNavigate } from 'react-router';
import './sync.css';

export function RecoveryPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const recovery = useRecoveryExport();
  const { status, code, saved, busy, error } = recovery;

  if (status === null) {
    return (
      <main className="tv-sync-main" aria-busy="true">
        <div className="tv-sync-skeleton-line" />
      </main>
    );
  }

  if (code !== null) {
    return (
      <main className="tv-sync-main">
        <h1 className="tv-sync-title">{tr('Write this down now.', '现在就把它抄写下来。')}</h1>
        <p className="tv-sync-subtitle">
          {tr(
            'The recovery code is shown only this once · it is stored nowhere',
            '恢复码只显示这一次 · 不会保存在任何地方',
          )}
        </p>
        <div className="tv-sync-empty">
          <p
            className="tv-sync-sas"
            aria-label={tr(`Recovery code ${code.code}`, `恢复码 ${code.code}`)}
          >
            {code.code}
          </p>
          <p className="tv-sync-lead">
            {tr(
              'This is the only time it is shown. It is not stored on this device, not in the backup, and not on the server — the server only holds a copy of your keys that nothing but this code can open.',
              '它只在此刻显示这一次。这台设备、备份和服务器上都不会保存它 —— 服务器只保存一份你的密钥副本，而只有这个恢复码能打开它。',
            )}
          </p>
          <p className="tv-sync-lead">
            {tr('You will also need the server address', '你还需要服务器地址')}{' '}
            <strong>{code.serverUrl}</strong> {tr('and the account id', '和账户 ID')}{' '}
            <strong>{code.accountId}</strong>
            {tr('. Keep them with the code.', '。请把它们与恢复码放在一起。')}
          </p>
          <p className="tv-sync-lead">
            {tr(
              'Lose the code but remember the master password: nothing is lost. Lose both: the vault contents cannot be recovered.',
              '丢失恢复码但记得主密码：数据无损。两者都丢失：保险库内容无法恢复。',
            )}
          </p>
          <div className="tv-sync-actions tv-sync-run-row">
            <button
              type="button"
              className="tv-sync-button"
              aria-pressed={saved}
              onClick={() => {
                recovery.setSaved(!saved);
              }}
            >
              {saved
                ? tr('Saved it somewhere safe', '已存放在安全的地方')
                : tr('I have written it down', '我已经抄写下来')}
            </button>
            <button
              type="button"
              className="tv-sync-primary"
              disabled={!saved}
              onClick={() => void navigate('/sync')}
            >
              {tr('Done', '完成')}
            </button>
          </div>
        </div>
      </main>
    );
  }

  return (
    <main className="tv-sync-main">
      <h1 className="tv-sync-title">{tr('Export a recovery code.', '导出恢复码。')}</h1>
      <p className="tv-sync-subtitle">
        {tr(
          'The only way back into the vault when every device is gone',
          '所有设备都丢失时，唯一能取回保险库的方式',
        )}
      </p>
      <div className="tv-sync-empty">
        <p className="tv-sync-lead">
          {tr(
            'A recovery code is the way back into your account if every paired device is gone. It is shown once, on the next screen, and generating a new one immediately stops the previous one from working.',
            '当所有已配对设备都丢失时，恢复码是回到账户的那条路。它只在下一个界面显示一次，并且生成新码会立即让旧码失效。',
          )}
        </p>
        {!status.vaultReady && (
          <p className="tv-sync-lead">
            {tr(
              'This account has no vault yet, so there is nothing for a recovery code to protect. Your ordinary snippets keep syncing without one.',
              '这个账户还没有保险库，恢复码也就没有需要保护的对象。你的普通片段无需恢复码即可继续同步。',
            )}
          </p>
        )}
        {status.vaultReady && !status.vaultUnlocked && (
          <p className="tv-sync-lead">
            {tr(
              'Unlock the vault first — the code wraps your vault key, so it can only be made while that key is in memory.',
              '请先解锁保险库 —— 恢复码封装的是你的保险库密钥，只有该密钥在内存中时才能生成。',
            )}
          </p>
        )}
        <div className="tv-sync-actions tv-sync-run-row">
          <button
            type="button"
            className="tv-sync-primary"
            disabled={busy || !recovery.canPublish}
            onClick={() => void recovery.publish()}
          >
            {tr('Generate and show it once', '生成并只显示一次')}
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

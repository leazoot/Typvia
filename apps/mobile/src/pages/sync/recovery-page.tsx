/**
 * Recovery code on a phone. The code is shown exactly once and
 * stored nowhere, so this screen says so before it generates one and refuses
 * to pretend it can show it again: leaving is blocked until the user states
 * they wrote it down.
 */
import { useTr } from '@typvia/ui';
import { useRecoveryExport } from '@typvia/ui/sync';
import { ScreenSkeleton, SettingsScreen } from '../settings/screen';
import './sync.css';

export function MobileRecoveryPage({ onBack }: { onBack: () => void }) {
  const tr = useTr();
  const recovery = useRecoveryExport();
  const { status, code, saved, busy, error } = recovery;

  if (status === null) return <ScreenSkeleton parent={tr('Sync', '同步')} onBack={onBack} />;

  if (code !== null) {
    return (
      <SettingsScreen
        parent={tr('Sync', '同步')}
        title={tr('Write this down now.', '现在就把它抄下来。')}
        // The back bar would leave without the code being written down, so it
        // is the same gate as the Done button rather than a way around it.
        onBack={saved ? onBack : () => undefined}
      >
        <p
          className="tv-sync-sas"
          aria-label={tr(`Recovery code ${code.code}`, `恢复码 ${code.code}`)}
        >
          {code.code}
        </p>
        <p className="tv-sync-lead">
          {tr(
            'This is the only time it is shown. It is not stored on this device, not in the backup, and not on the server — the server only holds a copy of your keys that nothing but this code can open.',
            '它只显示这一次。它不存在这台设备上,不在备份里,也不在服务器上 —— 服务器只保存一份你的密钥副本,而只有这个恢复码能打开它。',
          )}
        </p>
        <p className="tv-sync-lead">
          {tr('You will also need the server address', '你还需要服务器地址')}{' '}
          <strong>{code.serverUrl}</strong> {tr('and the account id', '和账户 ID')}{' '}
          <strong>{code.accountId}</strong>
          {tr('. Keep them with the code.', '。请把它们和恢复码放在一起。')}
        </p>
        <p className="tv-sync-lead">
          {tr(
            'Lose the code but remember the master password: nothing is lost. Lose both: the vault contents cannot be recovered.',
            '丢失恢复码但记得主密码:数据无损。两者都丢失:保险库内容无法恢复。',
          )}
        </p>
        <button
          type="button"
          className="tv-mset-button"
          aria-pressed={saved}
          onClick={() => {
            recovery.setSaved(!saved);
          }}
        >
          {saved
            ? tr('Saved it somewhere safe', '已妥善保存')
            : tr('I have written it down', '我已经抄下来了')}
        </button>
        <button
          type="button"
          className="tv-mset-button is-primary"
          disabled={!saved}
          onClick={onBack}
        >
          {tr('Done', '完成')}
        </button>
      </SettingsScreen>
    );
  }

  return (
    <SettingsScreen
      parent={tr('Sync', '同步')}
      title={tr('Export a recovery code.', '导出恢复码。')}
      onBack={onBack}
    >
      <p className="tv-sync-lead">
        {tr(
          'A recovery code is the way back into your account if every paired device is gone. It is shown once, on the next screen, and generating a new one immediately stops the previous one from working.',
          '当已配对的设备全部丢失时,恢复码是回到账户的唯一途径。它只在下一个页面显示一次;生成新的恢复码会立即让之前的失效。',
        )}
      </p>
      {!status.vaultReady && (
        <p className="tv-sync-lead">
          {tr(
            'This account has no vault yet, so there is nothing for a recovery code to protect. Your ordinary snippets keep syncing without one.',
            '这个账户还没有保险库,恢复码也就没有可保护的内容。你的普通片段没有它也照常同步。',
          )}
        </p>
      )}
      {status.vaultReady && !status.vaultUnlocked && (
        <p className="tv-sync-lead">
          {tr(
            'Unlock the vault first — the code wraps your vault key, so it can only be made while that key is in memory.',
            '请先解锁保险库 —— 恢复码要封装你的保险库密钥,只有该密钥在内存中时才能生成。',
          )}
        </p>
      )}
      <button
        type="button"
        className="tv-mset-button is-primary"
        disabled={busy || !recovery.canPublish}
        onClick={() => void recovery.publish()}
      >
        {tr('Generate and show it once', '生成并只显示一次')}
      </button>
      {error !== null && (
        <span className="tv-msync-notice" role="status">
          {error}
        </span>
      )}
    </SettingsScreen>
  );
}

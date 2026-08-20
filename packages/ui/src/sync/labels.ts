/**
 * The sentences and derived labels the sync surfaces read out. Pure functions
 * with no React and no IPC, so desktop and mobile say exactly the same thing:
 * the wording is part of the design (failure states name what is still fine
 * first; destructive prompts carry the real name), and a second copy of it
 * would drift on the first edit.
 */
import { IpcError, type Snippet, type SyncDevice, type SyncStatus } from '@typvia/shared';
import { trFor, type Locale } from '../i18n';
import { relativeTime } from './relative-time';

/**
 * Offline is not a failure: the host answers `unavailable` when it
 * could not reach the server, and every sync surface has to read that as "the
 * queue is waiting", never as "something broke".
 */
export function isOfflineError(caught: unknown): boolean {
  return caught instanceof IpcError && caught.code === 'unavailable';
}

/** The message of a failure the user should see, never a raw object. */
export function messageOf(caught: unknown, locale: Locale): string {
  return caught instanceof Error
    ? caught.message
    : trFor(locale)('Something went wrong.', '出了点问题。');
}

/** What an offline round means, said as reassurance rather than an error. */
export function offlineNotice(locale: Locale): string {
  return trFor(locale)(
    'Everything you saved is still here and still queued. Typvia will send it when the server answers.',
    '你保存的内容都还在，仍在发送队列中。服务器一有响应，Typvia 就会发送。',
  );
}

/** The Sync page headline: the count of devices the text lives on. */
export function headline(deviceCount: number, enabled: boolean, locale: Locale): string {
  const tr = trFor(locale);
  if (!enabled) return tr('Sync is off. Everything still works.', '同步已关闭。一切功能照常可用。');
  if (deviceCount === 0) return tr('Your text lives on this device.', '你的文本存放在这台设备上。');
  if (deviceCount === 1) return tr('Your text lives on one device.', '你的文本存放在一台设备上。');
  return tr(
    `Your text lives on ${String(deviceCount)} devices.`,
    `你的文本存放在 ${String(deviceCount)} 台设备上。`,
  );
}

/** What one finished round did, in the user's terms. */
export function roundNotice(
  pushed: number,
  applied: number,
  conflictCopies: number,
  locale: Locale,
): string {
  const tr = trFor(locale);
  if (conflictCopies > 0) {
    return tr(
      `${String(conflictCopies)} snippet${conflictCopies === 1 ? '' : 's'} changed in two places and need${conflictCopies === 1 ? 's' : ''} your decision.`,
      `${String(conflictCopies)} 个片段在两处被修改，需要你来决定。`,
    );
  }
  if (pushed === 0 && applied === 0) return tr('Already up to date.', '已是最新。');
  return tr(
    `Sent ${String(pushed)}, received ${String(applied)}.`,
    `已发送 ${String(pushed)} 条，接收 ${String(applied)} 条。`,
  );
}

/** Heading for the conflict card and page. */
export function conflictHeadline(count: number, locale: Locale): string {
  const tr = trFor(locale);
  return count === 1
    ? tr('One snippet changed in two places', '一个片段在两处被修改')
    : tr(`${String(count)} snippets changed in two places`, `${String(count)} 个片段在两处被修改`);
}

/**
 * The shape of a version in a conflict pair, without showing the body of a
 * secret: a sensitive pair is compared as two locked rows, so the label
 * describes size only.
 */
export function shapeLabel(snippet: Snippet, sensitive: boolean, locale: Locale): string {
  const tr = trFor(locale);
  if (sensitive || snippet.body === null) return tr('encrypted · not shown', '已加密 · 不显示');
  const lines = snippet.body === '' ? 0 : snippet.body.split('\n').length;
  const words = snippet.body.trim() === '' ? 0 : snippet.body.trim().split(/\s+/).length;
  return tr(
    `${String(lines)} line${lines === 1 ? '' : 's'} · ${String(words)} word${words === 1 ? '' : 's'}`,
    `${String(lines)} 行 · ${String(words)} 个词`,
  );
}

/**
 * This device's own row, built from the status alone. Used whenever the
 * server directory is unavailable — this device is on the account whatever
 * the server says, so the list still has something true to show.
 */
export function selfDevice(status: SyncStatus): SyncDevice {
  return {
    deviceId: status.deviceId,
    name: status.deviceName,
    platform: '',
    createdAt: 0,
    revokedAt: null,
    verified: true,
    isThisDevice: true,
    isRoot: false,
  };
}

/** The second line of a device row: role, trust and any pending decision. */
export function deviceRowMeta(
  device: SyncDevice,
  conflictCount: number,
  now: number,
  locale: Locale,
): string {
  const tr = trFor(locale);
  if (device.revokedAt !== null) {
    const when = relativeTime(device.revokedAt, now, locale);
    return tr(`revoked ${when}`, `已于${when}撤销`);
  }
  // Conflicts are awaiting a decision here, on this device — the record does
  // not say which device authored the other body, so the marker sits on the
  // row that owns the decision rather than guessing an attribution.
  if (device.isThisDevice && conflictCount > 0) {
    return tr(
      `${String(conflictCount)} conflict${conflictCount === 1 ? '' : 's'} to resolve`,
      `${String(conflictCount)} 个冲突待处理`,
    );
  }
  const role = device.isThisDevice
    ? tr('this device', '此设备')
    : device.isRoot
      ? tr('first device', '首台设备')
      : tr('paired', '已配对');
  // The platform is blank on the row built from the status alone (no server
  // directory yet); the separator would then dangle after the role.
  const parts = [role, device.platform, device.verified ? '' : tr('unverified', '未验证')];
  return parts.filter((part) => part !== '').join(' · ');
}

/** The status word on this device's own row. */
export function selfStatus(
  running: boolean,
  offline: boolean,
  lastSyncAt: number | null,
  now: number,
  locale: Locale,
): string {
  const tr = trFor(locale);
  // Offline wins: a round that cannot reach the server is not "syncing".
  if (offline) return tr('offline', '离线');
  if (running) return tr('syncing now', '正在同步');
  if (lastSyncAt === null) return tr('not synced yet', '尚未同步');
  return relativeTime(lastSyncAt, now, locale);
}

/** Confirmation sentence for a revoke — always with the device's real name. */
export function revokeConfirmation(name: string, locale: Locale): string {
  return trFor(locale)(
    `Revoke ${name}? It keeps the snippets it already has, and receives nothing after this. The sync key is rotated so its copy stops working.`,
    `撤销 ${name}？它保留已有的片段，此后不再收到任何内容。同步密钥会轮换，它持有的副本随即失效。`,
  );
}

/** What the user is told once a revoke went through. */
export function revokeNotice(name: string, locale: Locale): string {
  return trFor(locale)(
    `${name} can no longer read new snippets. The key was rotated.`,
    `${name} 已无法读取新片段。密钥已轮换。`,
  );
}

/** What the user is told once the key was rotated. */
export function rotateNotice(generation: number, locale: Locale): string {
  return trFor(locale)(
    `New key in use (generation ${String(generation)}). Existing snippets are unchanged.`,
    `新密钥已启用（第 ${String(generation)} 代）。已有片段不受影响。`,
  );
}

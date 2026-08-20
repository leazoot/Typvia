/**
 * The Sync group inside Settings: the switch, the plain facts and a manual
 * round. Both hosts show the same list — desktop as a settings group, mobile
 * as a settings screen — so the machine and its sentences live here.
 *
 * Anything spatial (the route, the device list, pairing, conflicts) belongs
 * to the Sync surface itself, not to this group.
 */
import { type SyncStatus, syncDisable, syncNow, syncResume, syncStatus } from '@typvia/shared';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { trFor, useLocale, type Locale } from '../i18n';

export interface SyncSettings {
  /** `null` until the first answer; the group renders nothing before that. */
  status: SyncStatus | null;
  busy: boolean;
  notice: string | null;
  refresh: () => Promise<void>;
  /** Turns sync off, or back on for the already-bound account. */
  toggle: () => Promise<void>;
  runNow: () => Promise<void>;
}

export function useSyncSettings(): SyncSettings {
  const locale = useLocale();
  const tr = useMemo(() => trFor(locale), [locale]);
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setStatus(await syncStatus());
  }, []);

  useEffect(() => {
    void refresh().catch(() => undefined);
  }, [refresh]);

  const toggle = useCallback(async () => {
    if (status === null) return;
    setBusy(true);
    setNotice(null);
    try {
      setStatus(status.enabled ? await syncDisable() : await syncResume());
    } catch (caught) {
      setNotice(
        caught instanceof Error
          ? caught.message
          : tr('The switch did not change.', '开关状态没有改变。'),
      );
    } finally {
      setBusy(false);
    }
  }, [status, tr]);

  const runNow = useCallback(async () => {
    setBusy(true);
    setNotice(null);
    try {
      const round = await syncNow();
      setNotice(
        tr(
          `Sent ${String(round.pushed)}, received ${String(round.applied)}.`,
          `已发送 ${String(round.pushed)} 条，已接收 ${String(round.applied)} 条。`,
        ),
      );
      await refresh();
    } catch (caught) {
      setNotice(
        caught instanceof Error
          ? tr(
              `${caught.message} Everything you saved is still here and still queued.`,
              `${caught.message} 你保存的内容都还在，仍在队列中等待发送。`,
            )
          : tr('Could not sync.', '同步未能完成。'),
      );
    } finally {
      setBusy(false);
    }
  }, [refresh, tr]);

  return { status, busy, notice, refresh, toggle, runNow };
}

/** The dot next to the group heading. Colour never carries the meaning on its
 * own — the label is always shown with it. */
export function syncBadge(
  status: SyncStatus,
  locale: Locale,
): {
  kind: 'success' | 'warning';
  label: string;
} {
  const tr = trFor(locale);
  if (!status.available) return { kind: 'warning', label: tr('Unavailable', '不可用') };
  if (!status.configured) return { kind: 'warning', label: tr('Not set up', '未设置') };
  if (!status.enabled) return { kind: 'warning', label: tr('Off', '已关闭') };
  if (status.conflictCount > 0) {
    return {
      kind: 'warning',
      label: tr(
        `${String(status.conflictCount)} to resolve`,
        `${String(status.conflictCount)} 项待处理`,
      ),
    };
  }
  return { kind: 'success', label: tr('On', '已开启') };
}

/** The backlog line: how much is still queued for the server. */
export function backlogLabel(pendingBacklog: number, locale: Locale): string {
  const tr = trFor(locale);
  return pendingBacklog === 0
    ? tr('Nothing waiting to send', '没有等待发送的内容')
    : tr(
        `${String(pendingBacklog)} change${pendingBacklog === 1 ? '' : 's'} waiting to send`,
        `${String(pendingBacklog)} 项变更等待发送`,
      );
}

/** Count phrasing used by the settings rows (the Sync page has its own,
 * sentence-leading form — see `conflictHeadline`). */
export function conflictCountLabel(count: number, locale: Locale): string {
  const tr = trFor(locale);
  return count === 1
    ? tr('1 snippet changed in two places', '1 个片段在两处被修改')
    : tr(`${String(count)} snippets changed in two places`, `${String(count)} 个片段在两处被修改`);
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  semanticModelDelete,
  semanticModelDownload,
  semanticStatus,
  semanticSync,
  type SemanticStatus,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';
import { GroupTitle, TextAction } from '../../paper/kit';
import { DangerAction, Said, SettingRow, State } from './settings-kit';

/** How often a moving download or index is re-read. */
const POLL_MS = 1200;

export function SemanticGroup() {
  const tr = useTr();
  const [status, setStatus] = useState<SemanticStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(
    () =>
      semanticStatus()
        .then(setStatus)
        .catch(() => undefined),
    [],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const indexing = status !== null && status.modelPresent && status.pendingCount > 0;
  const moving = status !== null && (status.downloading || indexing);
  useEffect(() => {
    if (!moving) return undefined;
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(timer);
  }, [moving, refresh]);

  const act = (action: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    action()
      .catch(() => undefined)
      .then(refresh)
      .finally(() => setBusy(false));
  };

  const pct =
    status !== null && status.downloadTotal > 0
      ? Math.round((status.downloadReceived / status.downloadTotal) * 100)
      : 0;
  const absent = status !== null && !status.modelPresent && !status.downloading;

  return (
    <section className="tvs-group">
      <GroupTitle>{tr('Semantic search', '语义搜索')}</GroupTitle>
      <SettingRow label={tr('Search by meaning', '按意思搜')}>
        {status?.downloadFailed === true && !status.downloading && (
          <Said>
            {tr(
              'Your library is untouched — the download did not finish. Try again when you are online.',
              '库没受影响——模型没下载完。联网后再试一次。',
            )}
          </Said>
        )}
        {status !== null && (
          <div className="tvs-inline">
            {status.modelPresent ? (
              <State kind="ok">{tr('On', '开着')}</State>
            ) : status.downloading ? (
              <State kind="idle">
                {tr(`Downloading · ${String(pct)}%`, `正在下载 · ${String(pct)}%`)}
              </State>
            ) : (
              <State kind="idle">{tr('Off', '没开')}</State>
            )}
            {absent && (
              <TextAction primary disabled={busy} onClick={() => act(semanticModelDownload)}>
                {status.downloadFailed
                  ? tr('Try again', '再试一次')
                  : tr('Download the model', '下载模型')}
              </TextAction>
            )}
          </div>
        )}
      </SettingRow>
      {status !== null && status.modelPresent && (
        <SettingRow label={tr('Local model', '本地模型')}>
          <div className="tvs-inline">
            <span className="tvs-text is-mono">{status.modelId}</span>
            {indexing ? (
              <span className="tvs-text is-meta">
                {tr(
                  `Indexing ${String(status.embeddedCount)} / ${String(status.embeddedCount + status.pendingCount)}`,
                  `正在索引 ${String(status.embeddedCount)} / ${String(status.embeddedCount + status.pendingCount)}`,
                )}
              </span>
            ) : (
              <State kind="ok">
                {tr(
                  `Ready · ${String(status.embeddedCount)} indexed`,
                  `就绪 · ${String(status.embeddedCount)} 条已索引`,
                )}
              </State>
            )}
            <TextAction disabled={busy || moving} onClick={() => act(semanticSync)}>
              {tr('Rebuild the index', '重建索引')}
            </TextAction>
            <DangerAction onClick={() => act(semanticModelDelete)}>
              {tr('Remove the model', '移除模型')}
            </DangerAction>
          </div>
        </SettingRow>
      )}
    </section>
  );
}

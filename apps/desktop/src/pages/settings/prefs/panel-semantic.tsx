// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Semantic search — three honest states: missing shows the
 * one CTA this page allows, downloading/indexing shows a quiet progress
 * thread, ready is a model row whose destructive action hides in the menu.
 */
import {
  semanticModelDelete,
  semanticModelDownload,
  semanticStatus,
  type SemanticStatus,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { PrefSection } from './pref-section';
import { OverflowMenu, SetLabel, SetRow, StatusWord } from './pref-kit';

export function SemanticPref() {
  const tr = useTr();
  const [status, setStatus] = useState<SemanticStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = () =>
    semanticStatus()
      .then(setStatus)
      .catch(() => undefined);

  useEffect(() => {
    void refresh();
  }, []);

  // Poll only while something is actually moving (download or indexing).
  const active =
    status !== null && (status.downloading || (status.modelPresent && status.pendingCount > 0));
  useEffect(() => {
    if (!active) return undefined;
    const timer = setInterval(() => void refresh(), 1200);
    return () => clearInterval(timer);
  }, [active]);

  const download = async () => {
    setBusy(true);
    try {
      await semanticModelDownload();
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await semanticModelDelete();
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const pct =
    status && status.downloadTotal > 0
      ? Math.round((status.downloadReceived / status.downloadTotal) * 100)
      : 0;
  const indexing = status?.modelPresent === true && status.pendingCount > 0;

  const sub = !status
    ? tr('Search by meaning, locally', '按含义查找,全程本机')
    : status.downloading
      ? tr(`Downloading model ${String(pct)}%`, `正在下载模型 ${String(pct)}%`)
      : !status.modelPresent
        ? tr('Enable local semantic search', '启用本地语义搜索')
        : indexing
          ? tr(
              `Indexing ${String(status.embeddedCount)} / ${String(status.embeddedCount + status.pendingCount)}`,
              `正在索引 ${String(status.embeddedCount)} / ${String(status.embeddedCount + status.pendingCount)}`,
            )
          : tr(
              `Local model · ${String(status.embeddedCount)} snippet${status.embeddedCount === 1 ? '' : 's'} indexed`,
              `本地模型 · ${String(status.embeddedCount)} 个片段已索引`,
            );

  return (
    <PrefSection
      id="semantic"
      glyph="semantic"
      name={tr('Semantic search', '语义搜索')}
      sub={
        status?.downloading || indexing ? (
          <>
            {sub}
            <span className="tvp-progress">
              <i
                style={{
                  width: status?.downloading
                    ? `${String(pct)}%`
                    : status && status.embeddedCount + status.pendingCount > 0
                      ? `${String(Math.round((status.embeddedCount / (status.embeddedCount + status.pendingCount)) * 100))}%`
                      : '0%',
                }}
              />
            </span>
          </>
        ) : (
          sub
        )
      }
      status={
        status?.modelPresent && !indexing ? (
          <StatusWord kind="ok">{tr('Ready', '就绪')}</StatusWord>
        ) : status?.downloadFailed ? (
          <StatusWord kind="warn">{tr('Download failed', '下载失败')}</StatusWord>
        ) : undefined
      }
    >
      {!status || (!status.modelPresent && !status.downloading) ? (
        <SetRow
          title={tr('Enable local semantic search', '启用本地语义搜索')}
          description={
            status?.downloadFailed
              ? tr(
                  'Your library is untouched — the download didn’t finish. Try again when you’re online.',
                  '你的库未受影响——下载未完成,联网后可重试。',
                )
              : tr(
                  'Find snippets by meaning. The model downloads once; everything stays on this Mac.',
                  '按含义查找片段。模型只下载一次,一切都留在这台 Mac。',
                )
          }
          actions={
            <button
              type="button"
              className="tvp-btn"
              disabled={busy}
              onClick={() => void download()}
            >
              {status?.downloadFailed ? tr('Try again', '重试') : tr('Download model', '下载模型')}
            </button>
          }
        />
      ) : (
        <>
          <SetLabel>{tr('Local model', '本地模型')}</SetLabel>
          <SetRow
            title={status.modelId}
            description={
              status.downloading
                ? tr(`Downloading ${String(pct)}%`, `下载中 ${String(pct)}%`)
                : indexing
                  ? tr(
                      `Indexing ${String(status.embeddedCount)} / ${String(status.embeddedCount + status.pendingCount)}`,
                      `正在索引 ${String(status.embeddedCount)} / ${String(status.embeddedCount + status.pendingCount)}`,
                    )
                  : tr(
                      `${String(status.embeddedCount)} snippet${status.embeddedCount === 1 ? '' : 's'} indexed · sensitive snippets are never embedded`,
                      `${String(status.embeddedCount)} 个片段已索引 · 敏感片段永不进入向量`,
                    )
            }
            status={
              status.modelPresent && !indexing ? (
                <StatusWord kind="ok">{tr('Ready', '就绪')}</StatusWord>
              ) : undefined
            }
            actions={
              status.modelPresent ? (
                <OverflowMenu
                  label={tr('Model actions', '模型操作')}
                  items={[
                    'divider',
                    {
                      label: tr('Remove model', '移除模型'),
                      danger: true,
                      onSelect: () => void remove(),
                    },
                  ]}
                />
              ) : undefined
            }
          />
        </>
      )}
    </PrefSection>
  );
}

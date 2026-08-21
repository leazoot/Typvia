// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { StatusDot, useTr } from '@typvia/ui';
import { useNavigate } from 'react-router';
import { splitTokens } from './editor-body';
import type { Draft } from './use-editor-draft';

interface EditorRailProps {
  draft: Draft;
  /** The saved snippet version; null until the first save. */
  version: number | null;
}

/**
 * The 360px right rail: "what will this produce, and where can I call it".
 * Shows the live preview (tokens resolve in accent), the trigger row, and —
 * once the snippet is saved — the version-history entry.
 */
export function EditorRail({ draft, version }: EditorRailProps) {
  const tr = useTr();
  const navigate = useNavigate();
  return (
    <aside className="tv-ed-rail" aria-label={tr('Snippet output', '片段输出')}>
      <div className="tv-ed-rail-scroll">
        <div className="tv-ed-rail-label">
          {tr('Preview', '预览')}
          <span className="tv-ed-rail-label-meta">{tr('as saved', '按已保存内容')}</span>
        </div>
        <div className="tv-ed-rail-preview">
          {draft.body === '' ? (
            <span className="tv-ed-rail-preview-empty">
              {tr('Nothing to preview yet.', '暂无可预览内容。')}
            </span>
          ) : (
            splitTokens(draft.body).map((part, index) =>
              part.token ? (
                <span key={index} className="tv-ed-rail-var">
                  {part.text.replace(/^\{\{\s*|\s*\}\}$/g, '')}
                </span>
              ) : (
                <span key={index}>{part.text}</span>
              ),
            )
          )}
        </div>

        <div className="tv-ed-rail-label tv-ed-rail-section">{tr('Call it from', '调用方式')}</div>
        <div className="tv-ed-rail-rows">
          <div className="tv-ed-rail-row">
            <span>{tr('Espanso trigger', 'Espanso 触发词')}</span>
            {draft.trigger === null ? (
              <span className="tv-ed-rail-row-meta">{tr('no trigger set', '未设置触发词')}</span>
            ) : (
              <span className="tv-ed-rail-row-trigger">{draft.trigger}</span>
            )}
          </div>
          <div className="tv-ed-rail-row">
            <span>{tr('Search', '搜索')}</span>
            <span className="tv-ed-rail-row-meta">
              {tr('by title, content and tags', '按标题、内容与标签')}
            </span>
          </div>
        </div>

        {draft.id !== null && version !== null && (
          <>
            <div className="tv-ed-rail-label tv-ed-rail-section">{tr('History', '版本历史')}</div>
            <div className="tv-ed-rail-rows">
              <div className="tv-ed-rail-row">
                <span>{tr('Current version', '当前版本')}</span>
                <button
                  type="button"
                  className="tv-ed-rail-history"
                  onClick={() => {
                    if (draft.id !== null) void navigate(`/editor/${draft.id}/history`);
                  }}
                >
                  {tr(`v${String(version)} · view history`, `v${String(version)} · 查看历史`)}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
      <div className="tv-ed-rail-foot">
        <StatusDot
          kind="secure"
          label={tr(
            'Not a secret — this snippet may be sent to AI.',
            '非 Secret——该片段可能会发送给 AI。',
          )}
        />
        <p>
          {tr(
            'Once made a Secret, it is masked by default and asks for verification.',
            '改为 Secret 后将默认遮挡并要求验证。',
          )}
        </p>
      </div>
    </aside>
  );
}

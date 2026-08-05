import { StatusDot } from '@typvia/ui';
import { splitTokens } from './editor-body';
import type { Draft } from './use-editor-draft';

interface EditorRailProps {
  draft: Draft;
}

/**
 * The 360px right rail (design 1c): "what will this produce, and where can
 * I call it". v1 shows the live preview (tokens resolve in accent) and the
 * trigger row; per-platform rows and the version lineage arrive with their
 * batches.
 */
export function EditorRail({ draft }: EditorRailProps) {
  return (
    <aside className="tv-ed-rail" aria-label="Snippet output">
      <div className="tv-ed-rail-scroll">
        <div className="tv-ed-rail-label">
          Preview · 实时预览
          <span className="tv-ed-rail-label-meta">as saved</span>
        </div>
        <div className="tv-ed-rail-preview">
          {draft.body === '' ? (
            <span className="tv-ed-rail-preview-empty">Nothing to preview yet.</span>
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

        <div className="tv-ed-rail-label tv-ed-rail-section">Call it from · 调用方式</div>
        <div className="tv-ed-rail-rows">
          <div className="tv-ed-rail-row">
            <span>Espanso trigger</span>
            {draft.trigger === null ? (
              <span className="tv-ed-rail-row-meta">no trigger set</span>
            ) : (
              <span className="tv-ed-rail-row-trigger">{draft.trigger}</span>
            )}
          </div>
          <div className="tv-ed-rail-row">
            <span>Search</span>
            <span className="tv-ed-rail-row-meta">by title, content and tags</span>
          </div>
        </div>
      </div>
      <div className="tv-ed-rail-foot">
        <StatusDot kind="secure" label="Not a secret — this snippet may be sent to AI." />
        <p lang="zh-Hans">改为 Secret 后将默认遮挡并要求验证。</p>
      </div>
    </aside>
  );
}

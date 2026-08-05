import type { Snippet } from '@typvia/shared';
import { TYPE_MARKS, TypeMark } from '@typvia/ui';
import { useNavigate } from 'react-router';

const MARK_BY_TYPE: Record<string, string> = {
  text: 'TX',
  markdown: 'TX',
  code: 'CD',
  command: 'CM',
  prompt: 'PR',
  template: 'TP',
  sensitive: 'SC',
  ai_action: 'AI',
  link: 'LK',
  temporary: 'TX',
};

/** Two-letter mark for a snippet type; unknown values read as plain text. */
export function markFor(snippetType: string): string {
  return MARK_BY_TYPE[snippetType] ?? 'TX';
}

interface PreviewProps {
  snippet: Snippet | null;
  folderName: string | null;
  /** Rendered as a drawer below 1360px; Close clears the selection there. */
  onClose: () => void;
}

/**
 * The 330px read-only preview pane (design 1b): a preview, never an edit
 * form — editing is the separate editor screen. Sensitive bodies never reach
 * this side of the IPC boundary, so nothing is rendered for them.
 */
export function LibraryPreview({ snippet, folderName, onClose }: PreviewProps) {
  const navigate = useNavigate();
  return (
    <aside
      aria-label="Snippet preview"
      className="tv-lib-preview"
      data-empty={snippet === null ? true : undefined}
    >
      {snippet !== null && (
        <div>
          <div className="tv-lib-preview-head">
            <TypeMark code={markFor(snippet.snippetType)} />
            <span className="tv-lib-preview-kind">
              {TYPE_MARKS[markFor(snippet.snippetType)] ?? 'Text'}
            </span>
            {snippet.trigger !== null && (
              <span className="tv-lib-preview-trigger">{snippet.trigger}</span>
            )}
            <button type="button" className="tv-lib-preview-close" onClick={onClose}>
              Close
            </button>
          </div>
          <h2 className="tv-lib-preview-title">{snippet.title}</h2>
          {snippet.description !== null && (
            <div className="tv-lib-preview-desc">{snippet.description}</div>
          )}
          {snippet.body !== null && <div className="tv-lib-preview-body">{snippet.body}</div>}
          <dl className="tv-lib-preview-meta">
            <div>
              <dt>Folder</dt>
              <dd>{folderName ?? 'Unsorted'}</dd>
            </div>
            <div>
              <dt>Used</dt>
              <dd>{snippet.usageCount === 1 ? '1 time' : `${String(snippet.usageCount)} times`}</dd>
            </div>
            <div>
              <dt>Versions</dt>
              <dd>{snippet.version}</dd>
            </div>
          </dl>
          <div className="tv-lib-preview-actions">
            <button
              type="button"
              className="tv-lib-preview-edit"
              onClick={() => void navigate(`/editor/${snippet.id}`)}
            >
              Edit
            </button>
          </div>
        </div>
      )}
    </aside>
  );
}

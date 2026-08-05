import { getSnippet, listFolderChildren } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { TypeMark } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router';
import { markFor } from '../library/preview';
import type { FolderEntry } from '../library/rail';
import { EditorBody } from './editor-body';
import { EditorRail } from './editor-rail';
import { EditorTabs } from './editor-tabs';
import { TestInsert } from './test-insert';
import { useEditorDraft, type SaveStatus } from './use-editor-draft';
import './editor.css';

const TYPE_WORDS: Record<string, string> = {
  text: 'Text',
  markdown: 'Markdown',
  code: 'Code',
  command: 'Command',
  prompt: 'Prompt',
  template: 'Template',
  ai_action: 'AI action',
  link: 'Link',
};

function statusText(status: SaveStatus, version: number | null, savedAt: number | null): string {
  if (status === 'saving') return 'Saving…';
  if (status === 'error') return 'Not saved';
  if (status === 'draft' || version === null) return 'Draft';
  const age = savedAt === null ? null : Date.now() - savedAt;
  const when =
    age === null || age < 60_000
      ? 'just now'
      : age < 3_600_000
        ? `${String(Math.floor(age / 60_000))} min ago`
        : 'earlier';
  return `Saved · v${String(version)} · ${when}`;
}

async function fetchFolderTree(): Promise<FolderEntry[]> {
  const entries: FolderEntry[] = [];
  async function walk(parentId: string | null, depth: number): Promise<void> {
    const children = await listFolderChildren(parentId);
    for (const folder of children) {
      entries.push({ folder, depth });
      await walk(folder.id, depth + 1);
    }
  }
  await walk(null, 0);
  return entries;
}

/** Router-state title carried from the Library's "Save as snippet" action. */
function draftTitleFrom(state: unknown): string {
  if (
    typeof state === 'object' &&
    state !== null &&
    'draftTitle' in state &&
    typeof state.draftTitle === 'string'
  ) {
    return state.draftTitle;
  }
  return '';
}

interface LoadedEditorProps {
  initial: Snippet | null;
  initialTitle: string;
  folders: FolderEntry[];
}

function LoadedEditor({ initial, initialTitle, folders }: LoadedEditorProps) {
  const navigate = useNavigate();
  const { draft, patch, status, errorMessage, version, savedAt, sensitiveKinds, savePulse } =
    useEditorDraft(initial, initialTitle);

  const folderName = folders.find(({ folder }) => folder.id === draft.folderId)?.folder.name;

  return (
    <main className="tv-ed">
      <div className="tv-ed-top">
        <nav aria-label="Breadcrumb" className="tv-ed-crumbs">
          <button
            type="button"
            className="tv-ed-crumb-link"
            onClick={() => void navigate('/library')}
          >
            Library
          </button>
          <span aria-hidden="true" className="tv-ed-crumb-sep">
            /
          </span>
          {folderName !== undefined && (
            <>
              <span className="tv-ed-crumb">{folderName}</span>
              <span aria-hidden="true" className="tv-ed-crumb-sep">
                /
              </span>
            </>
          )}
          <span className="tv-ed-crumb tv-ed-crumb-current">
            {draft.title === '' ? 'New snippet' : draft.title}
          </span>
        </nav>
        <div className="tv-ed-top-right">
          <TestInsert snippetId={draft.id} ready={status === 'saved' && draft.body.trim() !== ''} />
          <div className="tv-ed-status" data-status={status}>
            <span aria-hidden="true" className="tv-ed-status-dot" />
            <span role="status">{statusText(status, version, savedAt)}</span>
          </div>
        </div>
      </div>

      <div className="tv-ed-columns">
        <section className="tv-ed-main">
          <div className="tv-ed-head">
            <div className="tv-ed-head-meta">
              <TypeMark code={markFor(draft.snippetType)} />
              <span className="tv-ed-head-kind">{TYPE_WORDS[draft.snippetType] ?? 'Text'}</span>
              {draft.trigger !== null && (
                <span className="tv-ed-head-trigger">{draft.trigger}</span>
              )}
            </div>
            <input
              type="text"
              className="tv-ed-title"
              aria-label="Snippet title"
              placeholder="Untitled snippet"
              value={draft.title}
              onChange={(event) => {
                patch({ title: event.target.value });
              }}
            />
            <input
              type="text"
              className="tv-ed-desc"
              aria-label="Description"
              placeholder="描述(可选)"
              lang="zh-Hans"
              value={draft.description ?? ''}
              onChange={(event) => {
                const value = event.target.value;
                patch({ description: value === '' ? null : value });
              }}
            />
          </div>
          {/* key restarts the 140ms settle on every successful save */}
          <div
            key={savePulse}
            className="tv-ed-body-scroll"
            data-saved={savePulse > 0 || undefined}
          >
            <EditorBody
              value={draft.body}
              onChange={(value) => {
                patch({ body: value });
              }}
            />
          </div>
          <EditorTabs
            draft={draft}
            onPatch={patch}
            folders={folders}
            sensitiveKinds={sensitiveKinds}
            errorMessage={errorMessage}
          />
        </section>
        <EditorRail draft={draft} />
      </div>
    </main>
  );
}

/**
 * Snippet editor (design Phase 2 · 1c): content first — the body reads at
 * 14.5px/2.05 in the widest column; properties live in a bottom band;
 * the right rail answers "what will this produce and where can I call it".
 * Auto-saves quietly (no toast); the sensitive scan advises on save.
 */
export function EditorPage() {
  const { id } = useParams();
  const location = useLocation();
  const [initial, setInitial] = useState<Snippet | null>(null);
  const [ready, setReady] = useState(id === undefined);
  const [loadFailed, setLoadFailed] = useState(false);
  const [folders, setFolders] = useState<FolderEntry[]>([]);

  useEffect(() => {
    fetchFolderTree()
      .then(setFolders)
      .catch(() => {
        setFolders([]);
      });
  }, []);

  useEffect(() => {
    if (id === undefined) return;
    getSnippet(id)
      .then((snippet) => {
        setInitial(snippet);
        setReady(true);
      })
      .catch(() => {
        setLoadFailed(true);
      });
  }, [id]);

  if (loadFailed) {
    return (
      <main className="tv-ed tv-ed-missing">
        <p>Your library is intact — this snippet just isn't available.</p>
        <p lang="zh-Hans">片段库完好,只是该片段暂不可用。</p>
      </main>
    );
  }
  if (!ready) return <main className="tv-ed" aria-busy="true" />;
  return (
    <LoadedEditor
      initial={initial}
      initialTitle={draftTitleFrom(location.state)}
      folders={folders}
    />
  );
}

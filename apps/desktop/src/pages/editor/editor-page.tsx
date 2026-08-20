import { getSnippet, listFolderChildren } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { TypeMark, markForType, useTr, type Tr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router';
import { useEspanso } from '../../espanso/espanso-context';
import type { FolderEntry } from '../library/rail';
import { EditorBody } from './editor-body';
import { EditorRail } from './editor-rail';
import { EditorTabs } from './editor-tabs';
import { Organize } from './organize';
import { TestInsert } from './test-insert';
import { useEditorDraft, type SaveStatus } from './use-editor-draft';
import './editor.css';

const TYPE_WORDS: Record<string, readonly [string, string]> = {
  text: ['Text', '文本'],
  markdown: ['Markdown', 'Markdown'],
  code: ['Code', '代码'],
  command: ['Command', '命令'],
  prompt: ['Prompt', '提示词'],
  template: ['Template', '模板'],
  ai_action: ['AI action', 'AI 操作'],
  link: ['Link', '链接'],
};

function statusText(
  status: SaveStatus,
  version: number | null,
  savedAt: number | null,
  tr: Tr,
): string {
  if (status === 'saving') return tr('Saving…', '保存中…');
  if (status === 'error') return tr('Not saved', '未保存');
  if (status === 'draft' || version === null) return tr('Draft', '草稿');
  const age = savedAt === null ? null : Date.now() - savedAt;
  const when =
    age === null || age < 60_000
      ? tr('just now', '刚刚')
      : age < 3_600_000
        ? tr(
            `${String(Math.floor(age / 60_000))} min ago`,
            `${String(Math.floor(age / 60_000))} 分钟前`,
          )
        : tr('earlier', '更早');
  return tr(`Saved · v${String(version)} · ${when}`, `已保存 · v${String(version)} · ${when}`);
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
/** Reads a seeded draft value handed over by another screen's navigation. */
function draftValueFrom(state: unknown, key: 'draftTitle' | 'draftTrigger'): string {
  if (typeof state === 'object' && state !== null && key in state) {
    const value = (state as Record<string, unknown>)[key];
    if (typeof value === 'string') return value;
  }
  return '';
}

interface LoadedEditorProps {
  initial: Snippet | null;
  initialTitle: string;
  initialTrigger: string | null;
  folders: FolderEntry[];
}

function LoadedEditor({ initial, initialTitle, initialTrigger, folders }: LoadedEditorProps) {
  const tr = useTr();
  const navigate = useNavigate();
  const { draft, patch, status, errorMessage, version, savedAt, sensitiveKinds, savePulse } =
    useEditorDraft(initial, initialTitle, initialTrigger);

  // A saved snippet may add or change a trigger, so keep the espanso config in
  // sync (debounced, and only when the integration is on). Seeded with the
  // initial save time so opening an existing snippet does not trigger a sync.
  const { notifyMutation } = useEspanso();
  const lastSyncedAt = useRef<number | null>(savedAt);
  useEffect(() => {
    if (status === 'saved' && savedAt !== null && savedAt !== lastSyncedAt.current) {
      lastSyncedAt.current = savedAt;
      notifyMutation();
    }
  }, [status, savedAt, notifyMutation]);

  // Replays the 140ms settle on each save by restarting the CSS animation
  // in place. Never remount for this (e.g. a savePulse key): the quiet
  // auto-save fires mid-typing, and a remount steals the caret and scroll
  // position from under the user.
  const bodyScrollRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (savePulse === 0) return;
    const node = bodyScrollRef.current;
    if (node === null) return;
    node.removeAttribute('data-saved');
    // Style flush between remove and re-add restarts the animation.
    void node.offsetWidth;
    node.setAttribute('data-saved', '');
  }, [savePulse]);

  const folderName = folders.find(({ folder }) => folder.id === draft.folderId)?.folder.name;

  return (
    <main className="tv-ed">
      <div className="tv-ed-top">
        <nav aria-label={tr('Breadcrumb', '面包屑导航')} className="tv-ed-crumbs">
          <button
            type="button"
            className="tv-ed-crumb-link"
            onClick={() => void navigate('/library')}
          >
            {tr('Library', '片段库')}
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
            {draft.title === '' ? tr('New snippet', '新片段') : draft.title}
          </span>
        </nav>
        <div className="tv-ed-top-right">
          <TestInsert snippetId={draft.id} ready={status === 'saved' && draft.body.trim() !== ''} />
          <div className="tv-ed-status" data-status={status}>
            <span aria-hidden="true" className="tv-ed-status-dot" />
            <span role="status">{statusText(status, version, savedAt, tr)}</span>
          </div>
        </div>
      </div>

      <div className="tv-ed-columns">
        <section className="tv-ed-main">
          <div className="tv-ed-head">
            <div className="tv-ed-head-meta">
              <TypeMark code={markForType(draft.snippetType)} />
              <span className="tv-ed-head-kind">
                {tr(...(TYPE_WORDS[draft.snippetType] ?? TYPE_WORDS.text ?? ['Text', '文本']))}
              </span>
              {draft.trigger !== null && (
                <span className="tv-ed-head-trigger">{draft.trigger}</span>
              )}
            </div>
            <input
              type="text"
              className="tv-ed-title"
              aria-label={tr('Snippet title', '片段标题')}
              placeholder={tr('Untitled snippet', '未命名片段')}
              value={draft.title}
              onChange={(event) => {
                patch({ title: event.target.value });
              }}
            />
            <input
              type="text"
              className="tv-ed-desc"
              aria-label={tr('Description', '描述')}
              placeholder={tr('Description (optional)', '描述(可选)')}
              value={draft.description ?? ''}
              onChange={(event) => {
                const value = event.target.value;
                patch({ description: value === '' ? null : value });
              }}
            />
          </div>
          <div ref={bodyScrollRef} className="tv-ed-body-scroll">
            <EditorBody
              value={draft.body}
              onChange={(value) => {
                patch({ body: value });
              }}
            />
          </div>
          <Organize draft={draft} onApply={patch} />
          <EditorTabs
            draft={draft}
            onPatch={patch}
            folders={folders}
            sensitiveKinds={sensitiveKinds}
            errorMessage={errorMessage}
            saved={status === 'saved'}
          />
        </section>
        <EditorRail draft={draft} version={version} />
      </div>
    </main>
  );
}

/**
 * Snippet editor: content first — the body reads at
 * 14.5px/2.05 in the widest column; properties live in a bottom band;
 * the right rail answers "what will this produce and where can I call it".
 * Auto-saves quietly (no toast); the sensitive scan advises on save.
 */
export function EditorPage() {
  const tr = useTr();
  const { id } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
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
        <p>
          {tr(
            "Your library is intact — this snippet just isn't available.",
            '片段库完好,只是该片段暂不可用。',
          )}
        </p>
      </main>
    );
  }
  if (!ready) return <main className="tv-ed" aria-busy="true" />;
  // Sensitive snippets never open in this editor (the backend refuses the
  // save; the vault flow owns viewing and editing them).
  if (initial !== null && initial.securityLevel === 'sensitive') {
    return (
      <main className="tv-ed tv-ed-missing">
        <p>
          {tr(
            'Still safely encrypted — secret snippets are viewed and edited in the Vault.',
            '内容仍安全地加密着——密钥片段在保险库中查看与编辑。',
          )}
        </p>
        <button type="button" className="tv-ed-crumb-link" onClick={() => void navigate('/vault')}>
          {tr('Open Vault', '前往保险库')}
        </button>
      </main>
    );
  }
  return (
    <LoadedEditor
      initial={initial}
      initialTitle={draftValueFrom(location.state, 'draftTitle')}
      initialTrigger={draftValueFrom(location.state, 'draftTrigger') || null}
      folders={folders}
    />
  );
}

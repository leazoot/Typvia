import { IpcError, createSnippet, detectSensitive, updateSnippet } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { useCallback, useEffect, useRef, useState } from 'react';

/** Idle time after the last edit before the draft auto-saves. */
export const SAVE_DEBOUNCE_MS = 800;

/** Editable projection of a snippet; `id` is null until the first save. */
export interface Draft {
  id: string | null;
  title: string;
  body: string;
  snippetType: string;
  description: string | null;
  folderId: string | null;
  trigger: string | null;
  triggerMode: string | null;
  language: string | null;
  isFavorite: boolean;
  isPinned: boolean;
  isEnabled: boolean;
}

export type SaveStatus = 'draft' | 'saving' | 'saved' | 'error';

interface EditorDraft {
  draft: Draft;
  patch: (changes: Partial<Draft>) => void;
  status: SaveStatus;
  /** Business/user error from the last failed save (e.g. trigger conflict). */
  errorMessage: string | null;
  /** Version and timestamp of the last successful save. */
  version: number | null;
  savedAt: number | null;
  /** Advisory sensitive-pattern codes from the last save's body scan. */
  sensitiveKinds: string[];
  /** Bumps on every successful save — drives the 140ms save motion. */
  savePulse: number;
}

function draftFrom(snippet: Snippet): Draft {
  return {
    id: snippet.id,
    title: snippet.title,
    body: snippet.body ?? '',
    snippetType: snippet.snippetType,
    description: snippet.description,
    folderId: snippet.folderId,
    trigger: snippet.trigger,
    triggerMode: snippet.triggerMode,
    language: snippet.language,
    isFavorite: snippet.isFavorite,
    isPinned: snippet.isPinned,
    isEnabled: snippet.isEnabled,
  };
}

const EMPTY_DRAFT: Draft = {
  id: null,
  title: '',
  body: '',
  snippetType: 'text',
  description: null,
  folderId: null,
  trigger: null,
  triggerMode: null,
  language: null,
  isFavorite: false,
  isPinned: false,
  isEnabled: true,
};

/**
 * Draft state with debounced auto-save (create on first save, update after).
 * Saving is quiet by design: no toast — the status word and the 140ms body
 * settle are the only feedback. The sensitive scan runs on every successful
 * save and only ever advises; it never blocks the save.
 */
export function useEditorDraft(initial: Snippet | null): EditorDraft {
  const [draft, setDraft] = useState<Draft>(initial === null ? EMPTY_DRAFT : draftFrom(initial));
  const [status, setStatus] = useState<SaveStatus>(initial === null ? 'draft' : 'saved');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [version, setVersion] = useState<number | null>(initial?.version ?? null);
  const [savedAt, setSavedAt] = useState<number | null>(initial?.updatedAt ?? null);
  const [sensitiveKinds, setSensitiveKinds] = useState<string[]>([]);
  const [savePulse, setSavePulse] = useState(0);

  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latest = useRef(draft);
  latest.current = draft;
  // Serializes saves: a save started while another is in flight re-queues.
  const saving = useRef(false);
  const dirtyWhileSaving = useRef(false);

  const runSave = useCallback(async () => {
    const current = latest.current;
    if (current.title.trim() === '') return; // a title is required to persist
    if (saving.current) {
      dirtyWhileSaving.current = true;
      return;
    }
    saving.current = true;
    setStatus('saving');
    try {
      const saved =
        current.id === null
          ? await createSnippet({
              title: current.title,
              body: current.body,
              snippetType: current.snippetType,
              description: current.description,
              folderId: current.folderId,
              trigger: current.trigger,
              triggerMode: current.trigger === null ? null : (current.triggerMode ?? 'delimiter'),
              language: current.language,
            })
          : await updateSnippet({
              id: current.id,
              title: current.title,
              body: current.body,
              snippetType: current.snippetType,
              description: current.description,
              folderId: current.folderId,
              trigger: current.trigger,
              triggerMode: current.trigger === null ? null : (current.triggerMode ?? 'delimiter'),
              language: current.language,
              isFavorite: current.isFavorite,
              isPinned: current.isPinned,
              isEnabled: current.isEnabled,
            });
      if (current.id === null) {
        setDraft((previous) => ({ ...previous, id: saved.id }));
      }
      setVersion(saved.version);
      setSavedAt(saved.updatedAt);
      setStatus('saved');
      setErrorMessage(null);
      setSavePulse((n) => n + 1);
      // Advisory only: the save above already succeeded.
      detectSensitive(current.body)
        .then(setSensitiveKinds)
        .catch(() => {
          // Detection is a hint; a failed scan changes nothing.
        });
    } catch (error) {
      setStatus('error');
      setErrorMessage(error instanceof IpcError ? error.message : 'save failed');
    } finally {
      saving.current = false;
      if (dirtyWhileSaving.current) {
        dirtyWhileSaving.current = false;
        void runSave();
      }
    }
  }, []);

  const patch = useCallback(
    (changes: Partial<Draft>) => {
      setDraft((previous) => ({ ...previous, ...changes }));
      setStatus('draft');
      if (timer.current !== null) clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        void runSave();
      }, SAVE_DEBOUNCE_MS);
    },
    [runSave],
  );

  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  return { draft, patch, status, errorMessage, version, savedAt, sensitiveKinds, savePulse };
}

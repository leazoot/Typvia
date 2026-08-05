import { countSnippets, listSnippetPage } from '@typvia/shared';
import type { LibraryView, Snippet } from '@typvia/shared';
import { useCallback, useEffect, useRef, useState } from 'react';

export const PAGE_SIZE = 200;

/** The rail selection: one saved view, or one folder. */
export interface LibraryScope {
  view: LibraryView;
  folderId: string | null;
}

interface SnippetPages {
  /** Exact row count behind the current scope + type filter. */
  total: number;
  /** Row by absolute index; undefined while its page is loading. */
  rowAt: (index: number) => Snippet | undefined;
  /** Requests every page overlapping [first, last]; already-loaded pages are free. */
  ensureRange: (first: number, last: number) => void;
  /** Set when a page or the count failed to load. */
  failed: boolean;
  retry: () => void;
}

/**
 * Sparse page cache over the paged Library IPC. The virtual list asks for the
 * visible index range each render; only pages touching that range are ever
 * fetched, which is what keeps a 50k list light.
 */
export function useSnippetPages(scope: LibraryScope, snippetType: string | null): SnippetPages {
  const [pages, setPages] = useState<ReadonlyMap<number, Snippet[]>>(new Map());
  const [total, setTotal] = useState(0);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const pending = useRef<Set<number>>(new Set());
  // Stamps every in-flight request; bumping it makes stale responses no-ops.
  const generation = useRef(0);

  useEffect(() => {
    generation.current += 1;
    const requestGeneration = generation.current;
    pending.current.clear();
    setPages(new Map());
    setFailed(false);
    setTotal(0);
    countSnippets(scope.view, scope.folderId, snippetType)
      .then((count) => {
        if (generation.current === requestGeneration) setTotal(count);
      })
      .catch(() => {
        if (generation.current === requestGeneration) setFailed(true);
      });
  }, [scope.view, scope.folderId, snippetType, attempt]);

  const ensureRange = useCallback(
    (first: number, last: number) => {
      const requestGeneration = generation.current;
      const firstPage = Math.floor(first / PAGE_SIZE);
      const lastPage = Math.floor(Math.max(first, last) / PAGE_SIZE);
      for (let page = firstPage; page <= lastPage; page += 1) {
        if (pages.has(page) || pending.current.has(page)) continue;
        pending.current.add(page);
        listSnippetPage(scope.view, scope.folderId, snippetType, PAGE_SIZE, page * PAGE_SIZE)
          .then((rows) => {
            if (generation.current !== requestGeneration) return;
            setPages((previous) => new Map(previous).set(page, rows));
          })
          .catch(() => {
            if (generation.current === requestGeneration) setFailed(true);
          })
          .finally(() => {
            pending.current.delete(page);
          });
      }
    },
    [pages, scope.view, scope.folderId, snippetType],
  );

  const rowAt = useCallback(
    (index: number) => pages.get(Math.floor(index / PAGE_SIZE))?.[index % PAGE_SIZE],
    [pages],
  );

  const retry = useCallback(() => {
    setAttempt((n) => n + 1);
  }, []);

  return { total, rowAt, ensureRange, failed, retry };
}

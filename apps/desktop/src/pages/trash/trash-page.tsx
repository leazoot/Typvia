import {
  deleteSnippetForever,
  listFolderChildren,
  listTrash,
  purgeExpiredTrash,
  restoreSnippet,
} from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { Caret, TypeMark } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';
import { markFor } from '../library/preview';
import type { FolderEntry } from '../library/rail';
import './trash.css';

const PAGE_LIMIT = 500;
const RETENTION_DAYS = 30;
const DAY_MS = 24 * 60 * 60 * 1000;

/** Whole days before a trashed row is purged (never below zero). */
function daysLeft(deletedAt: number, now: number): number {
  const gone = deletedAt + RETENTION_DAYS * DAY_MS;
  return Math.max(0, Math.ceil((gone - now) / DAY_MS));
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

/**
 * Trash (no dedicated design artboard exists — built from the system
 * language: ledger rows, inline confirmation, red text with real counts,
 * caret-based empty state). Restore puts a snippet back exactly where it
 * was; forever-deletion always asks first, with the real number.
 */
export function TrashPage() {
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [folders, setFolders] = useState<FolderEntry[]>([]);
  const [checked, setChecked] = useState<ReadonlySet<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [failed, setFailed] = useState(false);

  const reload = useCallback(() => {
    listTrash(PAGE_LIMIT, 0)
      .then((trashed) => {
        setRows(trashed);
        setFailed(false);
      })
      .catch(() => {
        setFailed(true);
      });
  }, []);

  useEffect(() => {
    // The 30-day policy is enforced on entry; expired rows never render.
    purgeExpiredTrash()
      .catch(() => {
        // Purge failing must not block the listing.
      })
      .finally(reload);
    fetchFolderTree()
      .then(setFolders)
      .catch(() => {
        setFolders([]);
      });
  }, [reload]);

  const folderNames = new Map(folders.map(({ folder }) => [folder.id, folder.name]));
  const now = Date.now();

  const finishBatch = () => {
    setChecked(new Set());
    setConfirming(false);
    reload();
  };

  const restoreChecked = () => {
    Promise.allSettled([...checked].map((id) => restoreSnippet(id))).then(finishBatch, finishBatch);
  };

  const deleteCheckedForever = () => {
    Promise.allSettled([...checked].map((id) => deleteSnippetForever(id))).then(
      finishBatch,
      finishBatch,
    );
  };

  return (
    <main className="tv-trash">
      <div className="tv-trash-head">
        <h1 className="tv-trash-title">Trash</h1>
        <div className="tv-trash-sub" lang="zh-Hans">
          回收站
        </div>
        <div className="tv-trash-note">
          Items restore to exactly where they were. After {RETENTION_DAYS} days here they are
          deleted forever.
        </div>
      </div>

      <div className="tv-trash-list">
        {failed ? (
          <div className="tv-trash-state">
            <div className="tv-trash-state-title">Your library is unaffected.</div>
            <div className="tv-trash-state-text">The trash list failed to load just now.</div>
            <button type="button" className="tv-trash-state-action" onClick={reload}>
              Retry
            </button>
          </div>
        ) : rows !== null && rows.length === 0 ? (
          <div className="tv-trash-state">
            <div className="tv-trash-state-figure">
              <span className="tv-trash-state-slot" />
              <Caret height={16} />
            </div>
            <div className="tv-trash-state-title">Trash is empty</div>
            <div className="tv-trash-state-text">
              Deleted snippets wait here for {RETENTION_DAYS} days before they go for good.
            </div>
          </div>
        ) : (
          rows?.map((snippet) => (
            <div key={snippet.id} className="tv-trash-row">
              <input
                type="checkbox"
                className="tv-trash-check"
                aria-label={`Select ${snippet.title}`}
                checked={checked.has(snippet.id)}
                onChange={(event) => {
                  setChecked((previous) => {
                    const next = new Set(previous);
                    if (event.target.checked) next.add(snippet.id);
                    else next.delete(snippet.id);
                    return next;
                  });
                  setConfirming(false);
                }}
              />
              <TypeMark code={markFor(snippet.snippetType)} />
              <span className="tv-trash-row-title">{snippet.title}</span>
              <span className="tv-trash-row-folder">
                {snippet.folderId === null
                  ? 'Unsorted'
                  : (folderNames.get(snippet.folderId) ?? 'Unsorted')}
              </span>
              <span className="tv-trash-row-days">
                {snippet.deletedAt === null
                  ? ''
                  : `${String(daysLeft(snippet.deletedAt, now))} days left`}
              </span>
              <button
                type="button"
                className="tv-trash-row-restore"
                onClick={() => {
                  restoreSnippet(snippet.id).then(reload, reload);
                }}
              >
                Restore
              </button>
            </div>
          ))
        )}
      </div>

      <footer className="tv-trash-foot">
        {checked.size > 0 &&
          (confirming ? (
            <>
              <span className="tv-trash-foot-strong">
                Delete {checked.size} snippet{checked.size === 1 ? '' : 's'} forever? This cannot be
                undone.
              </span>
              <button
                type="button"
                className="tv-trash-foot-action tv-trash-foot-danger"
                onClick={deleteCheckedForever}
              >
                Delete {checked.size} snippet{checked.size === 1 ? '' : 's'}
              </button>
              <button
                type="button"
                className="tv-trash-foot-action"
                onClick={() => {
                  setConfirming(false);
                }}
              >
                Cancel
              </button>
            </>
          ) : (
            <>
              <span className="tv-trash-foot-strong">{checked.size} selected</span>
              <span className="tv-trash-foot-divider" />
              <button type="button" className="tv-trash-foot-action" onClick={restoreChecked}>
                Restore {checked.size} snippet{checked.size === 1 ? '' : 's'}
              </button>
              <button
                type="button"
                className="tv-trash-foot-action tv-trash-foot-danger"
                onClick={() => {
                  setConfirming(true);
                }}
              >
                Delete forever
              </button>
            </>
          ))}
        <span className="tv-trash-foot-count">
          {rows === null ? '' : `${String(rows.length)} in trash`}
        </span>
      </footer>
    </main>
  );
}

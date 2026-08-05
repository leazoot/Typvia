/**
 * The typed IPC layer — the frontend's ONLY data entry point. Components
 * never call `invoke` directly (enforced by the no-restricted-imports lint
 * rule); they use these functions, so transport, command names and error
 * normalization stay in one place and tests mock exactly here or below.
 */
import { invoke } from '@tauri-apps/api/core';
import { toIpcError } from './error';
import type {
  Folder,
  FolderCreateInput,
  FolderUpdateInput,
  LibraryCounts,
  LibraryView,
  SearchHit,
  Snippet,
  SnippetCreateInput,
  SnippetUpdateInput,
  Tag,
} from './types';

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw toIpcError(raw);
  }
}

export function createSnippet(input: SnippetCreateInput): Promise<Snippet> {
  return call('snippet_create', { input });
}

export function updateSnippet(input: SnippetUpdateInput): Promise<Snippet> {
  return call('snippet_update', { input });
}

export function getSnippet(id: string): Promise<Snippet> {
  return call('snippet_get', { id });
}

export function listSnippets(limit: number, offset: number): Promise<Snippet[]> {
  return call('snippet_list', { limit, offset });
}

export function listSnippetsByFolder(
  folderId: string | null,
  limit: number,
  offset: number,
): Promise<Snippet[]> {
  return call('snippet_list_by_folder', { folderId, limit, offset });
}

export function listSnippetPage(
  view: LibraryView,
  folderId: string | null,
  snippetType: string | null,
  limit: number,
  offset: number,
): Promise<Snippet[]> {
  return call('snippet_list_page', { view, folderId, snippetType, limit, offset });
}

export function countSnippets(
  view: LibraryView,
  folderId: string | null,
  snippetType: string | null,
): Promise<number> {
  return call('snippet_count', { view, folderId, snippetType });
}

export function libraryCounts(): Promise<LibraryCounts> {
  return call('library_counts');
}

export function trashSnippet(id: string): Promise<void> {
  return call('snippet_trash', { id });
}

export function restoreSnippet(id: string): Promise<void> {
  return call('snippet_restore', { id });
}

export function deleteSnippetForever(id: string): Promise<void> {
  return call('snippet_delete_forever', { id });
}

export function listTrash(limit: number, offset: number): Promise<Snippet[]> {
  return call('trash_list', { limit, offset });
}

export function purgeExpiredTrash(): Promise<number> {
  return call('trash_purge_expired');
}

export function createFolder(input: FolderCreateInput): Promise<Folder> {
  return call('folder_create', { input });
}

export function updateFolder(input: FolderUpdateInput): Promise<Folder> {
  return call('folder_update', { input });
}

export function deleteFolder(id: string): Promise<void> {
  return call('folder_delete', { id });
}

export function listFolderChildren(parentId: string | null): Promise<Folder[]> {
  return call('folder_list_children', { parentId });
}

export function createTag(name: string): Promise<Tag> {
  return call('tag_create', { name });
}

export function listTags(): Promise<Tag[]> {
  return call('tag_list');
}

export function renameTag(id: string, name: string): Promise<void> {
  return call('tag_rename', { id, name });
}

export function deleteTag(id: string): Promise<void> {
  return call('tag_delete', { id });
}

export function searchSnippets(query: string, limit: number, offset: number): Promise<SearchHit[]> {
  return call('search_snippets', { query, limit, offset });
}

/** Ranked search returning full snippet rows for the Library list. */
export function searchLibrary(query: string, limit: number): Promise<Snippet[]> {
  return call('search_library', { query, limit });
}

/**
 * Offline sensitive-content scan. Returns advisory pattern codes (never the
 * matched text); an empty array means nothing suspicious.
 */
export function detectSensitive(text: string): Promise<string[]> {
  return call('detect_sensitive', { text });
}

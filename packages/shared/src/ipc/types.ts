/**
 * IPC data shapes, mirroring the Rust DTOs in apps/desktop/src-tauri/src/dto.rs
 * (serde camelCase). v1 scope: normal snippets only — a sensitive snippet's
 * `body` is always `null` on this side of the boundary.
 */

export interface Snippet {
  id: string;
  title: string;
  body: string | null;
  snippetType: string;
  securityLevel: string;
  description: string | null;
  folderId: string | null;
  trigger: string | null;
  triggerMode: string | null;
  language: string | null;
  isFavorite: boolean;
  isPinned: boolean;
  isEnabled: boolean;
  createdAt: number;
  updatedAt: number;
  lastUsedAt: number | null;
  usageCount: number;
  version: number;
  deletedAt: number | null;
}

export interface SnippetCreateInput {
  title: string;
  body: string;
  snippetType: string;
  description?: string | null;
  folderId?: string | null;
  trigger?: string | null;
  triggerMode?: string | null;
  language?: string | null;
}

export interface SnippetUpdateInput {
  id: string;
  title: string;
  body: string;
  snippetType: string;
  description?: string | null;
  folderId?: string | null;
  trigger?: string | null;
  triggerMode?: string | null;
  language?: string | null;
  isFavorite: boolean;
  isPinned: boolean;
  isEnabled: boolean;
}

export interface Folder {
  id: string;
  parentId: string | null;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface FolderCreateInput {
  name: string;
  parentId?: string | null;
  sortOrder: number;
}

export interface FolderUpdateInput {
  id: string;
  name: string;
  parentId?: string | null;
  sortOrder: number;
}

export interface Tag {
  id: string;
  name: string;
  createdAt: number;
}

/** The Library rail vocabulary: four saved views plus single-folder scope. */
export type LibraryView = 'all' | 'recent' | 'starred' | 'unsorted' | 'folder';

export interface FolderCount {
  folderId: string;
  count: number;
}

export interface LibraryCounts {
  total: number;
  recent: number;
  starred: number;
  unsorted: number;
  folders: FolderCount[];
}

export type SearchTier = 'title_exact' | 'trigger' | 'title_prefix' | 'tag' | 'content';

export interface SearchHit {
  snippetId: string;
  title: string;
  tier: SearchTier;
  isSensitive: boolean;
}

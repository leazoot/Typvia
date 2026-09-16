// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { listFolderChildren, syncDevices, syncStatus, templateVariables } from '@typvia/shared';
import type { Folder, Snippet, SyncDevice, SyncStatus } from '@typvia/shared';
import { useCallback, useEffect, useRef, useState } from 'react';

/**
 * Every folder, parents before children. The library shows collections as one
 * flat level; folders nested by an older build keep their data and simply
 * appear side by side.
 */
export async function fetchFolders(): Promise<Folder[]> {
  const folders: Folder[] = [];
  async function walk(parentId: string | null): Promise<void> {
    for (const folder of await listFolderChildren(parentId)) {
      folders.push(folder);
      await walk(folder.id);
    }
  }
  await walk(null);
  return folders;
}

const keyOf = (snippet: Snippet) => `${snippet.id}:${String(snippet.version)}`;

/**
 * Variable names per snippet, asked of the template engine for the rows on
 * screen only and remembered per version, so scrolling back costs nothing.
 */
export function useVariables(
  snippets: readonly Snippet[],
): (snippet: Snippet) => readonly string[] | undefined {
  const [known, setKnown] = useState<ReadonlyMap<string, readonly string[]>>(new Map());
  const asked = useRef(new Set<string>());
  // The array is rebuilt every render; its keys are what identify it.
  const latest = useRef(snippets);
  latest.current = snippets;
  const wanted = snippets.map(keyOf).join('|');

  useEffect(() => {
    for (const snippet of latest.current) {
      const key = keyOf(snippet);
      if (asked.current.has(key) || snippet.body === null) continue;
      asked.current.add(key);
      templateVariables(snippet.body)
        .then((names) => setKnown((previous) => new Map(previous).set(key, names)))
        .catch(() => {
          // Unknown stays unknown: the row shows no variable tag rather than a wrong one.
        });
    }
  }, [wanted]);

  return useCallback((snippet: Snippet) => known.get(keyOf(snippet)), [known]);
}

export interface SyncPicture {
  status: SyncStatus | null;
  /** Devices still bound to the account, this one included. */
  devices: number;
  refresh: () => void;
}

export function useSyncPicture(): SyncPicture {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [devices, setDevices] = useState(0);

  const refresh = useCallback(() => {
    syncStatus()
      .then((next) => {
        setStatus(next);
        if (!next.enabled) return;
        syncDevices()
          .then((list: SyncDevice[]) =>
            setDevices(list.filter((device) => device.revokedAt === null).length),
          )
          .catch(() => setDevices(0));
      })
      .catch(() => setStatus(null));
  }, []);

  useEffect(refresh, [refresh]);
  return { status, devices, refresh };
}

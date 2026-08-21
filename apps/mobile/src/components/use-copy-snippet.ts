// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { copySnippet } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { designTokens, useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';

interface CopySnippet {
  /** Toast text while one is alive (toastLife duration), else null. */
  toast: string | null;
  copy: (snippet: Snippet) => void;
}

/**
 * Tap-to-copy for mobile strips. The write goes through the host, not
 * `navigator.clipboard`: the Android system WebView serves the app from a
 * non-secure origin and has no clipboard API there, and the host is also
 * where the usage count is recorded — a copy that does not count is a copy
 * that quietly breaks "recent first".
 *
 * A sensitive snippet's body is null on this side of the IPC boundary and its
 * copy must carry the auto-clear, so it takes the vault path (vault page)
 * instead; the tap is a no-op here.
 */
export function useCopySnippet(): CopySnippet {
  const tr = useTr();
  const [toast, setToast] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  const show = (message: string) => {
    setToast(message);
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      setToast(null);
    }, designTokens.motion.duration.toastLife);
  };

  const copy = (snippet: Snippet) => {
    if (snippet.body === null) return;
    copySnippet(snippet.id).then(
      () => {
        show(tr('Copied', '已复制'));
      },
      () => {
        // Failure states name what is still fine first (design rule).
        show(tr('Snippet unchanged — copy failed', '片段未受影响 —— 复制失败'));
      },
    );
  };

  return { toast, copy };
}

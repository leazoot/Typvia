// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { type RefObject, useEffect, useState } from 'react';

interface VirtualRowsOptions {
  /** Total row count backing the list. */
  count: number;
  /** Fixed row height in px — the whole reason no measuring library is needed. */
  rowHeight: number;
  /** Scrollable viewport element. */
  viewportRef: RefObject<HTMLElement | null>;
  /** Extra rows rendered beyond each edge of the viewport (default 8). */
  overscan?: number;
}

interface VirtualRows {
  /** First row index to render (inclusive). */
  first: number;
  /** Last row index to render (inclusive); -1 when the list is empty. */
  last: number;
  /** Height of the full (unrendered) list, for the scroll spacer. */
  totalHeight: number;
}

/**
 * Fixed-height list windowing. Rows are 52px by design contract precisely so
 * the Library virtualises to 50k items with plain arithmetic — no dynamic
 * measurement, hence no virtual-list dependency. Rendered rows are
 * absolutely positioned at `index * rowHeight` inside a spacer of
 * `totalHeight`.
 */
export function useVirtualRows({
  count,
  rowHeight,
  viewportRef,
  overscan = 8,
}: VirtualRowsOptions): VirtualRows {
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (viewport === null) return undefined;
    const readScroll = () => {
      setScrollTop(viewport.scrollTop);
    };
    const readHeight = () => {
      setViewportHeight(viewport.clientHeight);
    };
    readScroll();
    readHeight();
    viewport.addEventListener('scroll', readScroll, { passive: true });
    // jsdom has no ResizeObserver; the initial clientHeight read covers tests.
    const observer =
      typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(readHeight);
    observer?.observe(viewport);
    return () => {
      viewport.removeEventListener('scroll', readScroll);
      observer?.disconnect();
    };
  }, [viewportRef]);

  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const last = Math.min(count - 1, Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan);
  return { first, last, totalHeight: count * rowHeight };
}

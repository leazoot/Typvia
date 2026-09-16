// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Placement and dismissal for menus portaled out of the surface that opened
 * them. Geometry only, so no visual language lives here.
 */
import { useEffect, useRef } from 'react';

/** Read-only view of an element ref. */
type Anchor = { readonly current: HTMLElement | null };

export interface MenuPosition {
  left: number;
  top: number;
}

/** Kept clear of the window edges. */
const EDGE = 8;

/**
 * Pulls a point-anchored overlay — a right-click menu — back inside the window
 * when the click happened near an edge.
 */
export function keepInViewport(
  x: number,
  y: number,
  size: { width: number; height: number },
): MenuPosition {
  return {
    left: Math.min(x, Math.max(EDGE, window.innerWidth - size.width - EDGE)),
    top: Math.min(y, Math.max(EDGE, window.innerHeight - size.height - EDGE)),
  };
}

/**
 * Closes an overlay on an outside click or Escape, and on anything that would
 * strand it away from its trigger (scroll, resize) — a menu positioned against
 * the viewport cannot follow the row it belongs to.
 *
 * `inside` lists the elements that count as "not outside": with the menu in a
 * portal, its own node is no longer a descendant of the trigger's wrapper, and
 * a mousedown there must not dismiss it before the click lands.
 */
export function useMenuDismiss(open: boolean, close: () => void, inside: readonly Anchor[]): void {
  const parts = useRef(inside);
  parts.current = inside;
  useEffect(() => {
    if (!open) return;
    const away = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (!parts.current.some((part) => part.current?.contains(target) === true)) close();
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    document.addEventListener('mousedown', away);
    document.addEventListener('keydown', escape);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    return () => {
      document.removeEventListener('mousedown', away);
      document.removeEventListener('keydown', escape);
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', close, true);
    };
  }, [open, close]);
}

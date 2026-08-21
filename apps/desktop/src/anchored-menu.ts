// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Placement and dismissal for menus that must escape their own surface.
 *
 * Every surface that owns a ··· menu clips its content one way or another —
 * scrolling lists, folds that animate their height, rows that translate. A menu
 * laid out inside such a surface gets cut in half, so menus are portaled out to
 * the surface's palette root (see `paletteRoot`) and positioned against their
 * trigger in viewport coordinates. Shared by the workspace kit and the settings
 * prefs kit; it is geometry only, so neither layer's visual language leaks into
 * the other.
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react';

/** Read-only view of a ref — either kit's element refs satisfy it. */
type Anchor = { readonly current: HTMLElement | null };

export interface MenuPosition {
  left: number;
  top: number;
}

/** Kept clear of the window edges. */
const EDGE = 8;
/** Distance between the trigger and the menu. */
const GAP = 6;

/**
 * Positions an open menu next to its trigger, flipping above it when there is
 * no room below. Returns null on the first frame: the menu measures itself
 * before it is placed, so render it hidden until a position arrives.
 */
export function useMenuPosition(open: boolean, trigger: Anchor, menu: Anchor): MenuPosition | null {
  const [position, setPosition] = useState<MenuPosition | null>(null);
  useLayoutEffect(() => {
    if (!open) {
      setPosition(null);
      return;
    }
    const from = trigger.current?.getBoundingClientRect();
    const size = menu.current?.getBoundingClientRect();
    if (from === undefined || size === undefined) return;
    // Right-aligned with the trigger, then pulled back inside the window.
    const left = Math.min(
      Math.max(EDGE, from.right - size.width),
      Math.max(EDGE, window.innerWidth - size.width - EDGE),
    );
    const below = from.bottom + GAP;
    const above = from.top - size.height - GAP;
    const flip = below + size.height > window.innerHeight - EDGE && above > EDGE;
    setPosition({ left, top: flip ? above : below });
  }, [open, trigger, menu]);
  return position;
}

/**
 * Where a portaled menu mounts: the nearest ancestor that carries the surface's
 * palette (`.tvw-shell` for the workspace, `.tvp` for preferences). A menu has
 * to leave the subtree that clips it without leaving the one that colours it.
 */
export function paletteRoot(from: HTMLElement | null, selector: string): HTMLElement {
  return from?.closest<HTMLElement>(selector) ?? document.body;
}

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
      const target = event.target as Node;
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

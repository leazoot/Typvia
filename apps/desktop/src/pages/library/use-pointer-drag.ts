// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useCallback, useEffect, useRef, useState, type PointerEvent } from 'react';

/** How far a press travels before it becomes a drag rather than a click. */
const THRESHOLD_PX = 5;

export interface DropSpot {
  /** The `data-value` of the element under the pointer. */
  value: string;
  /** Upper quarter, middle, lower quarter — only asked for when zones are on. */
  zone: 'before' | 'on' | 'after';
}

export interface DragState<T> {
  item: T;
  x: number;
  y: number;
  spot: DropSpot | null;
}

function swallowClick(event: MouseEvent) {
  event.stopPropagation();
  event.preventDefault();
}

/**
 * Drag by pointer, not by the HTML drag API: the webview's own file-drop
 * handling can swallow HTML drags, and a pointer drag lets the drop target be
 * drawn exactly as the design has it. Drop targets are elements matching
 * `targets` that carry `data-value`. Escape cancels.
 */
export function usePointerDrag<T>({
  targets,
  zones = false,
  onDrop,
}: {
  targets: string;
  zones?: boolean;
  onDrop: (item: T, spot: DropSpot) => void;
}): { start: (event: PointerEvent, item: T) => void; drag: DragState<T> | null } {
  const [drag, setDrag] = useState<DragState<T> | null>(null);
  const press = useRef<{ item: T; x: number; y: number; dragging: boolean } | null>(null);
  const spot = useRef<DropSpot | null>(null);
  const latestDrop = useRef(onDrop);
  latestDrop.current = onDrop;

  useEffect(() => {
    const spotAt = (x: number, y: number): DropSpot | null => {
      if (typeof document.elementFromPoint !== 'function') return null;
      const target = document.elementFromPoint(x, y)?.closest<HTMLElement>(targets);
      const value = target?.dataset.value;
      if (target === null || target === undefined || value === undefined) return null;
      if (!zones) return { value, zone: 'on' };
      const rect = target.getBoundingClientRect();
      const ratio = (y - rect.top) / Math.max(rect.height, 1);
      return { value, zone: ratio < 0.25 ? 'before' : ratio > 0.75 ? 'after' : 'on' };
    };
    const end = () => {
      press.current = null;
      spot.current = null;
      setDrag(null);
      document.documentElement.classList.remove('tpi-dragging');
    };
    const move = (event: globalThis.PointerEvent) => {
      const current = press.current;
      if (current === null) return;
      if (
        !current.dragging &&
        Math.hypot(event.clientX - current.x, event.clientY - current.y) < THRESHOLD_PX
      ) {
        return;
      }
      if (!current.dragging) document.documentElement.classList.add('tpi-dragging');
      current.dragging = true;
      spot.current = spotAt(event.clientX, event.clientY);
      setDrag({ item: current.item, x: event.clientX, y: event.clientY, spot: spot.current });
    };
    const up = () => {
      const current = press.current;
      const landed = spot.current;
      if (current === null) return;
      if (!current.dragging) {
        press.current = null;
        return;
      }
      end();
      // The click that ends a drag is not a pick.
      window.addEventListener('click', swallowClick, true);
      setTimeout(() => window.removeEventListener('click', swallowClick, true), 0);
      if (landed !== null) latestDrop.current(current.item, landed);
    };
    const cancel = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && press.current?.dragging === true) end();
    };
    document.addEventListener('pointermove', move);
    document.addEventListener('pointerup', up);
    document.addEventListener('keydown', cancel);
    return () => {
      document.removeEventListener('pointermove', move);
      document.removeEventListener('pointerup', up);
      document.removeEventListener('keydown', cancel);
    };
  }, [targets, zones]);

  const start = useCallback((event: PointerEvent, item: T) => {
    if (event.button !== 0) return;
    press.current = { item, x: event.clientX, y: event.clientY, dragging: false };
  }, []);

  return { start, drag };
}

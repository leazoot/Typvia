// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The paper menu: the right-click menu on a row, and the window menu where
 * the system does not draw one. No icons — the left column belongs to the
 * single-choice dot alone. Destructive rows look like any other row until
 * the pointer or the keyboard lands on them.
 */
import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { keepInViewport, useMenuDismiss } from '../anchored-menu';
import './menu.css';

export type MenuEntry =
  | {
      kind: 'item';
      label: string;
      /** Shortcut as it is written on the key caps, e.g. "⌥⏎". */
      keys?: string;
      /** Absent (and no submenu) means the row is disabled. */
      onSelect?: () => void;
      danger?: boolean;
      /** Why a disabled row cannot be used, said on the row itself. */
      reason?: string;
      /** Set on every row of a single-choice group; true marks the current one. */
      chosen?: boolean;
      submenu?: readonly MenuEntry[];
    }
  | { kind: 'separator' }
  | { kind: 'heading'; label: string }
  /** A closing line that reports a number; never clickable. */
  | { kind: 'status'; label: string };

type ItemEntry = Extract<MenuEntry, { kind: 'item' }>;

export interface MenuPoint {
  x: number;
  y: number;
}

function usable(entry: MenuEntry | undefined): entry is ItemEntry {
  return entry?.kind === 'item' && (entry.onSelect !== undefined || entry.submenu !== undefined);
}

/** The next usable row from `from` in `direction`, wrapping; `from` when there is none. */
function stepFrom(entries: readonly MenuEntry[], from: number, direction: 1 | -1): number {
  const n = entries.length;
  const start = from < 0 ? (direction === 1 ? -1 : n) : from;
  for (let i = 1; i <= n; i += 1) {
    const index = (((start + direction * i) % n) + n) % n;
    if (usable(entries[index])) return index;
  }
  return from;
}

function Chevron() {
  return (
    <svg
      className="tpi-menu-chevron"
      width="6"
      height="9"
      viewBox="0 0 6 9"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M1 1l3.5 3.5L1 8"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function MenuPanel({
  entries,
  label,
  at,
  narrow,
  onDone,
  onBack,
}: {
  entries: readonly MenuEntry[];
  label: string;
  at: MenuPoint;
  narrow: boolean;
  /** Closes the whole menu. */
  onDone: () => void;
  /** Closes only this level; set on submenus. */
  onBack?: () => void;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const rows = useRef<Array<HTMLDivElement | null>>([]);
  const [spot, setSpot] = useState({ left: at.x, top: at.y });
  const [active, setActive] = useState(-1);
  const [keyboard, setKeyboard] = useState(false);
  const [sub, setSub] = useState<{ index: number; at: MenuPoint } | null>(null);

  useLayoutEffect(() => {
    const node = panel.current;
    if (node === null) return;
    setSpot(keepInViewport(at.x, at.y, node.getBoundingClientRect()));
    node.focus();
  }, [at.x, at.y]);

  const gutter = entries.some((entry) => entry.kind === 'item' && entry.chosen !== undefined);

  const openSub = (index: number) => {
    const rect = rows.current[index]?.getBoundingClientRect();
    setSub({ index, at: { x: (rect?.right ?? 0) + 4, y: (rect?.top ?? 0) - 9 } });
  };

  const activate = (index: number) => {
    const entry = entries[index];
    if (!usable(entry)) return;
    if (entry.submenu !== undefined) {
      openSub(index);
      return;
    }
    onDone();
    entry.onSelect?.();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // A submenu lives inside this panel's DOM; its keys stop with it.
    event.stopPropagation();
    setKeyboard(true);
    const current = entries[active];
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowUp':
        event.preventDefault();
        setActive((from) => stepFrom(entries, from, event.key === 'ArrowDown' ? 1 : -1));
        return;
      case 'ArrowRight':
        if (usable(current) && current.submenu !== undefined) {
          event.preventDefault();
          openSub(active);
        }
        return;
      case 'ArrowLeft':
        if (onBack !== undefined) {
          event.preventDefault();
          onBack();
        }
        return;
      case 'Escape':
        event.preventDefault();
        (onBack ?? onDone)();
        return;
      case 'Enter':
      case ' ':
        event.preventDefault();
        activate(active);
        return;
      default: {
        if (event.key.length !== 1 || event.metaKey || event.ctrlKey || event.altKey) return;
        const letter = event.key.toLowerCase();
        const n = entries.length;
        for (let i = 1; i <= n; i += 1) {
          const index = (Math.max(active, -1) + i + n) % n;
          const entry = entries[index];
          if (usable(entry) && entry.label.toLowerCase().startsWith(letter)) {
            setActive(index);
            return;
          }
        }
      }
    }
  };

  const subEntry = sub === null ? undefined : entries[sub.index];

  return (
    <div
      ref={panel}
      role="menu"
      aria-label={label}
      tabIndex={-1}
      className={`tpi tpi-menu${narrow ? ' is-narrow' : ''}${keyboard ? ' is-keyboard' : ''}`}
      style={{ left: spot.left, top: spot.top }}
      onKeyDown={onKeyDown}
      onMouseLeave={() => {
        if (sub === null) setActive(-1);
      }}
      onContextMenu={(event) => event.preventDefault()}
    >
      {entries.map((entry, index) => {
        if (entry.kind === 'separator') {
          return <div key={`sep-${String(index)}`} role="separator" className="tpi-menu-sep" />;
        }
        if (entry.kind === 'heading') {
          return (
            <div key={`head-${String(index)}`} role="presentation" className="tpi-menu-heading">
              <span aria-hidden="true" className="tpi-group-dot" />
              <span className="tpi-menu-heading-label">{entry.label}</span>
              <span aria-hidden="true" className="tpi-group-line" />
            </div>
          );
        }
        if (entry.kind === 'status') {
          return (
            <div key={`status-${String(index)}`} role="presentation" className="tpi-menu-status">
              {entry.label}
            </div>
          );
        }
        const enabled = usable(entry);
        const classes = ['tpi-menu-row'];
        if (index === active) classes.push('is-active');
        if (entry.danger === true) classes.push('is-danger');
        return (
          <div
            key={`${entry.label}-${String(index)}`}
            ref={(node) => {
              rows.current[index] = node;
            }}
            role={entry.chosen === undefined ? 'menuitem' : 'menuitemradio'}
            aria-checked={entry.chosen}
            aria-disabled={enabled ? undefined : true}
            aria-haspopup={entry.submenu === undefined ? undefined : 'menu'}
            aria-expanded={entry.submenu === undefined ? undefined : sub?.index === index}
            className={classes.join(' ')}
            onMouseEnter={() => {
              setKeyboard(false);
              if (!enabled) {
                setActive(-1);
                setSub(null);
                return;
              }
              setActive(index);
              if (entry.submenu !== undefined) openSub(index);
              else setSub(null);
            }}
            onClick={(event) => {
              event.stopPropagation();
              activate(index);
            }}
          >
            {gutter && (
              <span aria-hidden="true" className="tpi-menu-dot" data-on={entry.chosen === true} />
            )}
            <span className="tpi-menu-label">{entry.label}</span>
            {entry.reason !== undefined && <span className="tpi-menu-reason">{entry.reason}</span>}
            {entry.submenu !== undefined && <Chevron />}
            {entry.keys !== undefined && (
              <span aria-hidden="true" className="tpi-menu-keys">
                {entry.keys}
              </span>
            )}
          </div>
        );
      })}
      {sub !== null && subEntry?.kind === 'item' && subEntry.submenu !== undefined && (
        <MenuPanel
          entries={subEntry.submenu}
          label={subEntry.label}
          at={sub.at}
          narrow={narrow}
          onDone={onDone}
          onBack={() => {
            setSub(null);
            panel.current?.focus();
          }}
        />
      )}
    </div>
  );
}

/**
 * One menu at a time, opened at a point: `open` from a row's context-menu
 * event or from a key, and `node` rendered once. Closing hands focus back to
 * whatever had it.
 */
export function usePaperMenu(
  label: string,
  { narrow = true }: { narrow?: boolean } = {},
): {
  open: (at: MenuPoint, entries: readonly MenuEntry[]) => void;
  openAtPointer: (event: MouseEvent, entries: readonly MenuEntry[]) => void;
  isOpen: boolean;
  node: ReactNode;
} {
  const [state, setState] = useState<{ at: MenuPoint; entries: readonly MenuEntry[] } | null>(null);
  const holder = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);

  const close = useCallback(() => {
    setState(null);
    const target = returnFocus.current;
    returnFocus.current = null;
    if (target?.isConnected === true) target.focus();
  }, []);
  useMenuDismiss(state !== null, close, [holder]);

  const open = useCallback((at: MenuPoint, entries: readonly MenuEntry[]) => {
    returnFocus.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setState({ at, entries });
  }, []);

  return {
    open,
    openAtPointer: (event, entries) => {
      event.preventDefault();
      open({ x: event.clientX, y: event.clientY }, entries);
    },
    isOpen: state !== null,
    node:
      state === null
        ? null
        : createPortal(
            <div ref={holder} className="tpi-menu-holder">
              <MenuPanel
                entries={state.entries}
                label={label}
                at={state.at}
                narrow={narrow}
                onDone={close}
              />
            </div>,
            document.body,
          ),
  };
}

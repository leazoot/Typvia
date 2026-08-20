/**
 * Shared parts of the workspace interaction language.
 * Pure presentation — no IPC here. Glyphs are drawn on a 20px grid with a
 * 1.5px stroke that never scales, so no icon library enters the product.
 */
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { keepInViewport, paletteRoot, useMenuDismiss, useMenuPosition } from '../anchored-menu';

export type GlyphName = 'search' | 'gear';

const GLYPH_PATHS: Record<GlyphName, ReactNode> = {
  search: (
    <>
      <circle cx="9" cy="9" r="5.4" />
      <path d="M12.9 12.9 16.6 16.6" />
    </>
  ),
  gear: (
    <>
      <circle cx="10" cy="10" r="2.6" />
      <path d="M10 3.4v2M10 14.6v2M3.4 10h2M14.6 10h2M5.3 5.3l1.4 1.4M13.3 13.3l1.4 1.4M14.7 5.3l-1.4 1.4M6.7 13.3l-1.4 1.4" />
    </>
  ),
};

export function Glyph({ name }: { name: GlyphName }) {
  return (
    <span aria-hidden="true" className="tvw-glyph">
      <svg viewBox="0 0 20 20">{GLYPH_PATHS[name]}</svg>
    </span>
  );
}

/**
 * Two-character mono mark. Screen readers get the full word; the characters
 * themselves are decoration (the library's `TypeMark` covers snippet types —
 * this one carries the rooms, which are not snippet types).
 */
export function Mark({ code, label }: { code: string; label: string }) {
  return (
    <span className="tvw-mark" role="img" aria-label={label}>
      <span aria-hidden="true">{code}</span>
    </span>
  );
}

export function Dot({ kind }: { kind: 'ok' | 'warn' | 'idle' }) {
  return <span aria-hidden="true" className={`tvw-dot ${kind === 'idle' ? '' : kind}`} />;
}

/** The trigger token — Typvia's own visual asset, never a colour badge. */
export function TriggerToken({ trigger }: { trigger: string }) {
  return (
    <span className="tvw-trigger">
      <span aria-hidden="true" className="tvw-trigger-mark">
        ⌁
      </span>
      {trigger}
    </span>
  );
}

/**
 * A list row: the whole row is activatable, yet the contextual actions inside
 * it stay real buttons (so no button ever nests in a button).
 */
export interface RowModifiers {
  /** ⌘ (or Ctrl) was held — pages read it as "select", not "open". */
  meta: boolean;
  shift: boolean;
}

export function Row({
  className = '',
  label,
  onOpen,
  onContextMenu,
  children,
}: {
  className?: string;
  label: string;
  onOpen?: (modifiers: RowModifiers) => void;
  onContextMenu?: (event: MouseEvent) => void;
  children: ReactNode;
}) {
  const keys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (onOpen === undefined) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onOpen({ meta: event.metaKey || event.ctrlKey, shift: event.shiftKey });
    }
  };
  return (
    <div
      className={`tvw-row ${className}`}
      role={onOpen === undefined ? undefined : 'button'}
      tabIndex={onOpen === undefined ? undefined : 0}
      aria-label={onOpen === undefined ? undefined : label}
      onClick={(event) => onOpen?.({ meta: event.metaKey || event.ctrlKey, shift: event.shiftKey })}
      onKeyDown={keys}
      onContextMenu={onContextMenu}
    >
      {children}
    </div>
  );
}

/** A contextual action inside a row — text only, revealed on hover. */
export function Action({
  label,
  cta = false,
  disabled = false,
  onRun,
}: {
  label: string;
  cta?: boolean;
  disabled?: boolean;
  onRun: () => void;
}) {
  return (
    <button
      type="button"
      className={`tvw-act ${cta ? 'is-cta' : ''}`}
      disabled={disabled}
      onClick={(event) => {
        event.stopPropagation();
        onRun();
      }}
    >
      {label}
    </button>
  );
}

/**
 * Copy with spatial feedback: the button itself becomes "Copied" for a
 * moment. No toast anywhere in the product for this.
 */
export function CopyAction({
  label,
  doneLabel,
  onCopy,
}: {
  label: string;
  doneLabel: string;
  onCopy: () => Promise<void>;
}) {
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );
  return (
    <button
      type="button"
      className={`tvw-act ${done ? 'is-done' : ''}`}
      onClick={(event) => {
        event.stopPropagation();
        void onCopy().then(() => {
          setDone(true);
          timer.current = setTimeout(() => setDone(false), 1200);
        });
      }}
    >
      {done ? doneLabel : label}
    </button>
  );
}

export interface MenuItem {
  label: string;
  danger?: boolean;
  onSelect: () => void;
}

export type MenuEntry = MenuItem | 'divider';

function MenuItems({ items, onDone }: { items: readonly MenuEntry[]; onDone: () => void }) {
  return (
    <>
      {items.map((item, index) =>
        item === 'divider' ? (
          <hr key={`d-${String(index)}`} />
        ) : (
          <button
            key={item.label}
            type="button"
            role="menuitem"
            className={item.danger === true ? 'danger' : ''}
            onClick={(event) => {
              event.stopPropagation();
              onDone();
              item.onSelect();
            }}
          >
            {item.label}
          </button>
        ),
      )}
    </>
  );
}

/** Low-frequency and destructive actions live behind ··· , never in the row. */
export function OverflowMenu({ label, items }: { label: string; items: readonly MenuEntry[] }) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  const menu = useRef<HTMLSpanElement>(null);
  const close = useCallback(() => setOpen(false), []);
  useMenuDismiss(open, close, [wrap, menu]);
  const position = useMenuPosition(open, wrap, menu);

  return (
    <span ref={wrap} className="tvw-menu-wrap">
      <button
        type="button"
        className="tvw-act"
        aria-label={label}
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((current) => !current);
        }}
      >
        ···
      </button>
      {open &&
        createPortal(
          <span
            ref={menu}
            className="tvw-menu is-fixed"
            role="menu"
            style={position ?? { left: 0, top: 0, visibility: 'hidden' }}
          >
            <MenuItems items={items} onDone={close} />
          </span>,
          paletteRoot(wrap.current, '.tvw-shell'),
        )}
    </span>
  );
}

/**
 * Right-click menus. One instance per page: `open(event, items)` from a
 * row's `onContextMenu`, and render `node` once at the page root.
 */
export function useContextMenu(label: string): {
  open: (event: MouseEvent, items: readonly MenuEntry[]) => void;
  node: ReactNode;
} {
  const [state, setState] = useState<{
    x: number;
    y: number;
    items: readonly MenuEntry[];
    root: HTMLElement;
  } | null>(null);
  const wrap = useRef<HTMLSpanElement>(null);
  const close = useCallback(() => setState(null), []);
  useMenuDismiss(state !== null, close, [wrap]);

  // A right-click near an edge would otherwise open the menu off-screen.
  useLayoutEffect(() => {
    if (state === null || wrap.current === null) return;
    const spot = keepInViewport(state.x, state.y, wrap.current.getBoundingClientRect());
    if (spot.left !== state.x || spot.top !== state.y) {
      setState({ ...state, x: spot.left, y: spot.top });
    }
  }, [state]);

  return {
    open: (event, items) => {
      event.preventDefault();
      setState({
        x: event.clientX,
        y: event.clientY,
        items,
        root: paletteRoot(event.target as HTMLElement, '.tvw-shell'),
      });
    },
    node:
      state === null
        ? null
        : createPortal(
            <span
              ref={wrap}
              className="tvw-menu is-fixed"
              role="menu"
              aria-label={label}
              style={{ left: state.x, top: state.y }}
            >
              <MenuItems items={state.items} onDone={close} />
            </span>,
            state.root,
          ),
  };
}

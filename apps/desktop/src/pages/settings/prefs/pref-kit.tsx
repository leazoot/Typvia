// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Building blocks of the redesigned preferences surface:
 * status dots, contextual chips, segmented control, overflow menu, in-panel
 * folds and the Typvia glyph set. Pure presentation — no IPC in this file.
 */
import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { paletteRoot, useMenuDismiss, useMenuPosition } from '../../../anchored-menu';

export type DotKind = 'ok' | 'warn' | 'idle';

export function Dot({ kind }: { kind: DotKind }) {
  return <span aria-hidden="true" className={`tvp-dot ${kind === 'idle' ? '' : kind}`} />;
}

/** Right-aligned status of a row: a dot plus one word. */
export function StatusWord({ kind, children }: { kind: DotKind; children: ReactNode }) {
  return (
    <span className="tvp-status-inline">
      <Dot kind={kind} />
      {children}
    </span>
  );
}

export function Chip({
  cta = false,
  disabled = false,
  onClick,
  children,
}: {
  cta?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`tvp-chip ${cta ? 'cta' : ''}`}
      disabled={disabled}
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
    >
      {children}
    </button>
  );
}

export function SetLabel({ children }: { children: ReactNode }) {
  return <div className="tvp-label">{children}</div>;
}

/** A quiet setting row: title/description left, status and hover actions right. */
export function SetRow({
  title,
  description,
  status,
  actions,
  onClick,
  children,
}: {
  title?: ReactNode;
  description?: ReactNode;
  status?: ReactNode;
  /** Revealed on hover only (contextual actions). */
  actions?: ReactNode;
  onClick?: () => void;
  children?: ReactNode;
}) {
  const Tag = onClick ? 'button' : 'div';
  return (
    <Tag
      type={onClick ? 'button' : undefined}
      className="tvp-set"
      style={
        onClick
          ? {
              width: '100%',
              border: 0,
              background: 'none',
              font: 'inherit',
              color: 'inherit',
              textAlign: 'left',
              cursor: 'pointer',
            }
          : undefined
      }
      onClick={onClick}
    >
      <span className="tvp-grow">
        {title !== undefined && <span className="tvp-set-t">{title}</span>}
        {description !== undefined && <span className="tvp-set-d">{description}</span>}
        {children}
      </span>
      {actions !== undefined && <span className="tvp-hover-actions">{actions}</span>}
      {status}
    </Tag>
  );
}

/* ---- segmented control with a sliding indicator ---- */
export function Seg<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: ReadonlyArray<{ id: T; label: string }>;
  value: T;
  onChange: (next: T) => void;
}) {
  const wrap = useRef<HTMLSpanElement>(null);
  const [ind, setInd] = useState<{ x: number; w: number } | null>(null);

  useLayoutEffect(() => {
    const active = wrap.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]');
    if (active) setInd({ x: active.offsetLeft - 2, w: active.offsetWidth });
  }, [value, options]);

  return (
    <span ref={wrap} className="tvp-seg" role="group" aria-label={label}>
      {ind && (
        <span
          aria-hidden="true"
          className="tvp-seg-ind"
          style={{ transform: `translateX(${String(ind.x)}px)`, width: `${String(ind.w)}px` }}
        />
      )}
      {options.map((option) => (
        <button
          key={option.id}
          type="button"
          aria-pressed={option.id === value}
          onClick={() => onChange(option.id)}
        >
          {option.label}
        </button>
      ))}
    </span>
  );
}

/* ---- overflow menu: low-frequency and destructive actions live here ---- */
export interface MenuItem {
  label: string;
  danger?: boolean;
  onSelect: () => void;
}

export function OverflowMenu({ label, items }: { label: string; items: (MenuItem | 'divider')[] }) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  const menu = useRef<HTMLSpanElement>(null);
  const close = useCallback(() => setOpen(false), []);
  // The panels this menu lives in animate their height and therefore clip
  // their content; the menu is placed against the window instead.
  useMenuDismiss(open, close, [wrap, menu]);
  const position = useMenuPosition(open, wrap, menu);

  return (
    <span ref={wrap} className="tvp-menu-wrap">
      <button
        type="button"
        className="tvp-chip"
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
            className="tvp-menu is-fixed"
            role="menu"
            style={position ?? { left: 0, top: 0, visibility: 'hidden' }}
          >
            {items.map((item, index) =>
              item === 'divider' ? (
                <hr key={`d-${String(index)}`} />
              ) : (
                <button
                  key={item.label}
                  type="button"
                  role="menuitem"
                  className={item.danger ? 'danger' : ''}
                  onClick={(event) => {
                    event.stopPropagation();
                    close();
                    item.onSelect();
                  }}
                >
                  {item.label}
                </button>
              ),
            )}
          </span>,
          paletteRoot(wrap.current, '.tvp'),
        )}
    </span>
  );
}

/* ---- in-panel fold (Advanced, full paths, restore…) ---- */
export function Fold({ open, children }: { open: boolean; children: ReactNode }) {
  return (
    <div className={`tvp-fold ${open ? 'open' : ''}`}>
      <div>
        <div className="tvp-fold-inner">{children}</div>
      </div>
    </div>
  );
}

/** A quiet toggle row driving a {@link Fold} below it. */
export function FoldRow({
  label,
  open,
  onToggle,
}: {
  label: ReactNode;
  open: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className="tvp-set"
      style={{
        width: '100%',
        border: 0,
        background: 'none',
        font: 'inherit',
        color: 'inherit',
        textAlign: 'left',
        cursor: 'pointer',
      }}
      aria-expanded={open}
      onClick={onToggle}
    >
      <span className="tvp-grow">
        <span className="tvp-set-t" style={{ color: 'var(--tvp-secondary)' }}>
          {label}
        </span>
      </span>
      <span aria-hidden="true" className={`tvp-fold-row-chev ${open ? 'open' : ''}`}>
        ›
      </span>
    </button>
  );
}

export function KV({ rows }: { rows: ReadonlyArray<[ReactNode, ReactNode]> }) {
  return (
    <dl className="tvp-kv">
      {rows.map(([term, detail], index) => (
        <span key={index} style={{ display: 'contents' }}>
          <dt>{term}</dt>
          <dd>{detail}</dd>
        </span>
      ))}
    </dl>
  );
}

/* ---- Typvia glyphs: 20px grid, 1.5px stroke, squarish, no icon library ---- */
export type GlyphName =
  'general' | 'engine' | 'semantic' | 'browser' | 'sync' | 'ai' | 'import' | 'data';

const GLYPH_PATHS: Record<GlyphName, ReactNode> = {
  general: (
    <>
      <path d="M4 14.5 8 5.5l4 9M5.4 11.5h5.2" />
      <path d="M13.5 14.5c0-2.6 3-2.6 3-4.7 0-1-.7-1.8-1.7-1.8-.8 0-1.3.4-1.6 1" />
    </>
  ),
  engine: <path d="M11.5 3 6 11h3.6L8.5 17 14 9h-3.6z" />,
  semantic: (
    <>
      <rect x="3.5" y="3.5" width="8.4" height="8.4" rx="2.4" />
      <path d="M11.6 11.6 16.5 16.5" />
    </>
  ),
  browser: (
    <>
      <circle cx="10" cy="10" r="6.5" />
      <path d="M3.5 10h13M10 3.5c-3.4 3.8-3.4 9.2 0 13 3.4-3.8 3.4-9.2 0-13Z" />
    </>
  ),
  sync: (
    <>
      <path d="M16 8.5a6.2 6.2 0 0 0-11.6-1.4M4 11.5a6.2 6.2 0 0 0 11.6 1.4" />
      <path d="M16 4.5v4h-4M4 15.5v-4h4" />
    </>
  ),
  ai: (
    <path d="M10 3.5c.5 3.4 3.1 6 6.5 6.5-3.4.5-6 3.1-6.5 6.5-.5-3.4-3.1-6-6.5-6.5 3.4-.5 6-3.1 6.5-6.5Z" />
  ),
  import: (
    <>
      <path d="M10 3.5v9.5M6.2 9.6l3.8 3.9 3.8-3.9" />
      <path d="M4 16.5h12" />
    </>
  ),
  data: (
    <>
      <rect x="3.5" y="3.5" width="13" height="13" rx="3" />
      <rect x="7" y="7" width="6" height="6" rx="1.4" />
    </>
  ),
};

export function Glyph({ name }: { name: GlyphName }) {
  return (
    <span aria-hidden="true" className="tvp-glyph">
      <svg viewBox="0 0 20 20">{GLYPH_PATHS[name]}</svg>
    </span>
  );
}

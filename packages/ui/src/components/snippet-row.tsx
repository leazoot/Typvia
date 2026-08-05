import type { ReactNode } from 'react';
import { designTokens } from '../tokens';
import { Caret } from './caret';
import { TypeMark } from './type-mark';
import './snippet-row.css';

const ROW_HEIGHT = designTokens.space.rowHeightDesktop;

interface ListTrayProps {
  children: ReactNode;
  className?: string;
}

/**
 * The sunken tray behind list rows. Position-relative so the selection
 * plate can travel inside it.
 */
export function ListTray({ children, className }: ListTrayProps) {
  return <div className={className ? `tv-list-tray ${className}` : 'tv-list-tray'}>{children}</div>;
}

interface SelectionPlateProps {
  /** Zero-based index of the selected row; null hides the plate. */
  index: number | null;
}

/**
 * The lifted selection surface: ONE absolutely-positioned element translated
 * on Y at 140ms (selection travel), never a per-row class — that is what
 * makes the selection read as a plate that travels. Light theme lifts to
 * paper with the e1-row shadow; graphite lifts two steps in lightness
 * instead (shadows read as nothing there).
 */
export function SelectionPlate({ index }: SelectionPlateProps) {
  if (index === null) return null;
  return (
    <div
      aria-hidden="true"
      className="tv-selection-plate"
      data-testid="selection-plate"
      style={{ transform: `translateY(${index * ROW_HEIGHT}px)` }}
    />
  );
}

interface SnippetRowProps {
  mark: string;
  title: string;
  /** CN subtitle column (design rule: EN main label + CN subtitle). */
  cn?: string;
  /** Mono content preview; hidden from screen readers (announced via title/type). */
  preview?: string;
  trigger?: string;
  /** Folder-name column (right-aligned, Library layout). */
  folder?: string;
  selected?: boolean;
  /** Label next to ↵ when selected, e.g. "Open". */
  selectedAction?: string;
  /** Row activation (single select / open). */
  onClick?: () => void;
  /** Batch-selection checkbox state; rendered only when onCheckedChange exists. */
  checked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
}

/**
 * 52px fixed-height snippet row (fixed so the list can virtualise). At rest
 * the right slot shows the trigger; selected, it shows ↵ + action + caret.
 * Selection styling lives on the SelectionPlate, not here.
 */
export function SnippetRow({
  mark,
  title,
  cn,
  preview,
  trigger,
  folder,
  selected = false,
  selectedAction = 'Open',
  onClick,
  checked = false,
  onCheckedChange,
}: SnippetRowProps) {
  return (
    <div className="tv-row" data-selected={selected || undefined} onClick={onClick}>
      {onCheckedChange !== undefined && (
        <input
          type="checkbox"
          className="tv-row-check"
          aria-label={`Select ${title}`}
          checked={checked}
          onClick={(event) => {
            event.stopPropagation();
          }}
          onChange={(event) => {
            onCheckedChange(event.target.checked);
          }}
        />
      )}
      <TypeMark code={mark} />
      <span className="tv-row-title">{title}</span>
      {cn !== undefined && (
        <span className="tv-row-cn" lang="zh-Hans">
          {cn}
        </span>
      )}
      {preview !== undefined && (
        <span aria-hidden="true" className="tv-row-preview">
          {preview}
        </span>
      )}
      {folder !== undefined && <span className="tv-row-folder">{folder}</span>}
      {selected ? (
        <span className="tv-row-open">
          <span className="tv-row-return">↵</span>
          {selectedAction}
          <Caret height={14} />
        </span>
      ) : (
        trigger !== undefined && <span className="tv-row-trigger">{trigger}</span>
      )}
    </div>
  );
}

/**
 * Loading placeholder for a not-yet-fetched row: static text-shaped bars,
 * never a spinner (design rule). Same fixed height as a real row.
 */
export function SnippetRowSkeleton() {
  return (
    <div aria-hidden="true" className="tv-row tv-row-skeleton">
      <span className="tv-row-skeleton-mark" />
      <span className="tv-row-skeleton-bar tv-row-skeleton-title" />
      <span className="tv-row-skeleton-bar tv-row-skeleton-line" />
    </div>
  );
}

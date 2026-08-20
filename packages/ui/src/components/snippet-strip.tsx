import type { ReactNode } from 'react';
import './snippet-strip.css';

interface SnippetStripProps {
  title: ReactNode;
  /** Single-line truncated body preview; hidden from screen readers. */
  preview?: ReactNode;
  /** Machine content (commands, code, templates, prompts) previews in mono. */
  previewMono?: boolean;
  /** Locked vault rows show a dots line where the preview would be. */
  previewDots?: boolean;
  /** Bottom caps line — type word plus recency, e.g. "COMMAND · 2 MIN". */
  meta?: ReactNode;
  /** Right-column glyph: return hint, usage count or a LOCKED label. */
  trailing?: ReactNode;
  /** Honest no-op state (a locked sensitive strip before the vault opens). */
  disabled?: boolean;
  onPress?: () => void;
}

/**
 * Editorial snippet strip (design: typvia_mobile_export): no card, no icon,
 * no type chip — title, preview and a mono caps meta line separated by
 * hairlines. Content is the interface; hierarchy comes from size and greys.
 * A disabled strip stays focusable and announced (aria-disabled), because
 * its locked state is information, not absence.
 */
export function SnippetStrip({
  title,
  preview,
  previewMono = false,
  previewDots = false,
  meta,
  trailing,
  disabled = false,
  onPress,
}: SnippetStripProps) {
  return (
    <button
      type="button"
      className="tv-strip"
      aria-disabled={disabled || undefined}
      onClick={disabled ? undefined : onPress}
    >
      <span className="tv-strip-main">
        <span className="tv-strip-title">{title}</span>
        {previewDots ? (
          <span aria-hidden="true" className="tv-strip-dots">
            ••••••••••
          </span>
        ) : (
          preview !== undefined && (
            <span
              aria-hidden="true"
              className={previewMono ? 'tv-strip-preview is-mono' : 'tv-strip-preview'}
            >
              {preview}
            </span>
          )
        )}
        {meta !== undefined && <span className="tv-strip-meta">{meta}</span>}
      </span>
      {trailing !== undefined && <span className="tv-strip-trailing">{trailing}</span>}
    </button>
  );
}

/**
 * Loading placeholder strip: static text-shaped bars, never a spinner
 * (design rule). Same rhythm as a real strip.
 */
export function SnippetStripSkeleton() {
  return (
    <div aria-hidden="true" className="tv-strip tv-strip-skeleton">
      <span className="tv-strip-main">
        <span className="tv-strip-skeleton-bar tv-strip-skeleton-title" />
        <span className="tv-strip-skeleton-bar tv-strip-skeleton-preview" />
      </span>
    </div>
  );
}

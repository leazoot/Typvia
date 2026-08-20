import type { Snippet } from '@typvia/shared';
import type { ReactNode } from 'react';
import { SnippetStrip, useLocale, useTr } from '@typvia/ui';
import { previewIsMono, stripMeta, typeWord } from './format';

interface SnippetStripRowProps {
  snippet: Snippet;
  /** Row tap — the Recall flow opens the snippet detail screen. */
  onOpen: (snippet: Snippet) => void;
  /** 'recency' = "Command · 2 min" (Home); 'type' = word only (Search);
   *  'none' = no meta line (Library rows carry a usage count instead). */
  meta?: 'recency' | 'type' | 'none';
  /** Right-column glyph — Search's ↵ hint or Library's usage count. */
  trailing?: ReactNode;
}

/**
 * One Snippet DTO rendered as an editorial strip, shared by Home, Search and
 * Library. A sensitive snippet arrives with a null body: it renders as a
 * locked row — dots for a preview, a LOCKED trailing label — and stays inert
 * (honestly disabled) until the vault surface unlocks it; the app never
 * routes a locked row into the plain detail screen.
 */
export function SnippetStripRow({
  snippet,
  onOpen,
  meta = 'recency',
  trailing,
}: SnippetStripRowProps) {
  const tr = useTr();
  const locale = useLocale();
  const locked = snippet.body === null;

  const metaText =
    locked || meta === 'none'
      ? undefined
      : meta === 'type'
        ? typeWord(snippet.snippetType, locale)
        : stripMeta(snippet.snippetType, snippet.lastUsedAt, Date.now(), locale);

  return (
    <SnippetStrip
      title={snippet.title}
      preview={locked ? undefined : (snippet.body ?? undefined)}
      previewMono={previewIsMono(snippet.snippetType)}
      previewDots={locked}
      meta={metaText}
      trailing={
        locked ? <span className="tv-strip-locked">{tr('Locked', '已锁定')}</span> : trailing
      }
      disabled={locked}
      onPress={() => {
        onOpen(snippet);
      }}
    />
  );
}

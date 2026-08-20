import { searchSnippetsAll } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { Caret, SnippetStrip, useLocale, useTr } from '@typvia/ui';
import type { ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';
import { previewIsMono, typeWord } from '../../components/format';
import { SearchLineInput } from '../../components/search-line';
import { pushBackHandler } from '../../platform';
import './search.css';

const SEARCH_LIMIT = 200;

interface SearchPageProps {
  onCancel: () => void;
  /** Row tap opens the snippet detail screen; locked rows never fire it. */
  onOpen: (snippet: Snippet) => void;
}

/** First case-insensitive occurrence underlined in accent. */
function highlight(text: string, query: string): ReactNode {
  const needle = query.trim().toLowerCase();
  if (needle === '') return text;
  const at = text.toLowerCase().indexOf(needle);
  if (at === -1) return text;
  return (
    <>
      {text.slice(0, at)}
      <span className="tv-msearch-hit">{text.slice(at, at + needle.length)}</span>
      {text.slice(at + needle.length)}
    </>
  );
}

/**
 * Search screen: the line goes live —
 * accent underline, mono query, Cancel — and results replace on every
 * keystroke, grouped Snippets / Templates / Vault. Vault matches surface by
 * title only, as locked rows; their bodies are never in any index.
 */
export function SearchPage({ onCancel, onOpen }: SearchPageProps) {
  const tr = useTr();
  const locale = useLocale();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const generation = useRef(0);

  // Android system back leaves the search screen, same as Cancel.
  useEffect(() => pushBackHandler(onCancel), [onCancel]);

  useEffect(() => {
    generation.current += 1;
    const stamp = generation.current;
    if (query.trim() === '') {
      setResults(null);
      setFailed(false);
      return;
    }
    searchSnippetsAll(query, SEARCH_LIMIT)
      .then((hits) => {
        if (generation.current === stamp) setResults(hits);
      })
      .catch(() => {
        if (generation.current === stamp) setFailed(true);
      });
  }, [query]);

  const snippets =
    results?.filter((s) => s.securityLevel === 'normal' && s.snippetType !== 'template') ?? [];
  const templates =
    results?.filter((s) => s.securityLevel === 'normal' && s.snippetType === 'template') ?? [];
  const vault = results?.filter((s) => s.securityLevel !== 'normal') ?? [];

  const renderRow = (snippet: Snippet, locked: boolean) => (
    <li key={snippet.id}>
      <SnippetStrip
        title={highlight(snippet.title, query)}
        preview={locked || snippet.body === null ? undefined : highlight(snippet.body, query)}
        previewMono={previewIsMono(snippet.snippetType)}
        previewDots={locked}
        meta={locked ? undefined : typeWord(snippet.snippetType, locale)}
        trailing={locked ? <span className="tv-strip-locked">{tr('Locked', '已锁定')}</span> : '↵'}
        disabled={locked}
        onPress={() => {
          onOpen(snippet);
        }}
      />
    </li>
  );

  return (
    <main className="tv-mobile-page tv-msearch">
      <div className="tv-msearch-line">
        <SearchLineInput
          value={query}
          onChange={setQuery}
          placeholder={tr('Search snippets, commands, prompts', '搜索片段、命令、提示词')}
          label={tr('Search snippets', '搜索片段')}
          autoFocus
          onCancel={onCancel}
        />
      </div>

      {failed ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-title">
            {tr('Your snippets are safe on this device.', '你的片段在这台设备上安然无恙。')}
          </div>
          <div className="tv-mobile-state-text">
            {tr('Search failed to answer just now — try again.', '搜索刚才没有响应，请再试一次。')}
          </div>
        </div>
      ) : results === null ? null : results.length === 0 ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-figure">
            <span className="tv-mobile-state-slot" />
            <Caret height={16} />
          </div>
          <div className="tv-mobile-state-title">
            {tr(`No snippet matches “${query}”`, `没有片段匹配“${query}”`)}
          </div>
          <div className="tv-mobile-state-text">
            {tr('Try fewer letters, or clear the search.', '试试更少的字母，或清除搜索。')}
          </div>
        </div>
      ) : (
        <>
          {snippets.length > 0 && (
            <section aria-label={tr('Snippets', '片段')} className="tv-msearch-group">
              <div className="tv-msearch-group-head">
                <span className="tv-mobile-label">{tr('Snippets', '片段')}</span>
                <span className="tv-msearch-count" role="status" aria-live="polite">
                  {snippets.length}
                </span>
              </div>
              <ul className="tv-mobile-strips tv-strip-list">
                {snippets.map((s) => renderRow(s, false))}
              </ul>
            </section>
          )}
          {templates.length > 0 && (
            <section aria-label={tr('Templates', '模板')} className="tv-msearch-group">
              <div className="tv-msearch-group-head">
                <span className="tv-mobile-label">{tr('Templates', '模板')}</span>
              </div>
              <ul className="tv-mobile-strips tv-strip-list">
                {templates.map((s) => renderRow(s, false))}
              </ul>
            </section>
          )}
          {vault.length > 0 && (
            <section aria-label={tr('Vault', '保险库')} className="tv-msearch-group">
              <div className="tv-msearch-group-head">
                <span className="tv-mobile-label">{tr('Vault', '保险库')}</span>
              </div>
              <ul className="tv-mobile-strips tv-strip-list">
                {vault.map((s) => renderRow(s, true))}
              </ul>
            </section>
          )}
        </>
      )}
    </main>
  );
}

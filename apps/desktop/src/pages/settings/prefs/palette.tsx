/**
 * Settings command palette (⌘K): grouped, instantly filtered, lands on the
 * matching section (opened exclusively). Replaces the permanent search box
 * the design forbids.
 */
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { usePrefs } from './pref-section';

export interface PaletteEntry {
  group: string;
  label: string;
  sectionId: string;
}

export function SettingsPalette({
  entries,
  open,
  onClose,
}: {
  entries: ReadonlyArray<PaletteEntry>;
  open: boolean;
  onClose: () => void;
}) {
  const tr = useTr();
  const { openOnly } = usePrefs();
  const [query, setQuery] = useState('');
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery('');
      const timer = setTimeout(() => input.current?.focus(), 50);
      return () => clearTimeout(timer);
    }
    return undefined;
  }, [open]);

  if (!open) return null;

  const hits = entries.filter((entry) =>
    (entry.group + entry.label).toLowerCase().includes(query.toLowerCase()),
  );
  const groups = [...new Set(hits.map((entry) => entry.group))];

  const go = (sectionId: string) => {
    onClose();
    openOnly(sectionId);
    const section = document.querySelector(`[data-pref="${sectionId}"]`);
    if (section && typeof section.scrollIntoView === 'function') {
      section.scrollIntoView({ block: 'center' });
    }
  };

  return (
    <>
      <div className="tvp-scrim" onMouseDown={onClose} />
      <div className="tvp-palette" role="dialog" aria-label={tr('Search settings', '搜索设置')}>
        <input
          ref={input}
          type="text"
          value={query}
          placeholder={tr('Search settings…', '搜索设置…')}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setQuery(event.target.value)}
        />
        <div className="tvp-palette-results">
          {hits.length === 0 ? (
            <div className="tvp-palette-empty">
              {tr(`Nothing matches “${query}”`, `没有匹配「${query}」的设置`)}
            </div>
          ) : (
            groups.map((group) => (
              <div key={group}>
                <div className="tvp-palette-group">{group}</div>
                {hits
                  .filter((entry) => entry.group === group)
                  .map((entry) => (
                    <button
                      key={`${entry.group}-${entry.label}`}
                      type="button"
                      className="tvp-palette-item"
                      onClick={() => go(entry.sectionId)}
                    >
                      {entry.label}
                      <span className="tvp-palette-where">{entry.group}</span>
                    </button>
                  ))}
              </div>
            ))
          )}
        </div>
      </div>
    </>
  );
}

import { Caret } from './caret';
import './search-line.css';

/** The three desktop scales; each maps to its type-scale role in tokens.json. */
export type SearchLineScale = 'hero' | 'library' | 'panel';

const CARET_HEIGHT: Record<SearchLineScale, number> = {
  hero: 40,
  library: 24,
  panel: 17,
};

interface SearchLineProps {
  scale: SearchLineScale;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  /** Accessible name for the input (the search line has no visible label). */
  label: string;
}

/**
 * The search line — the product's signature. A 1.5px ink underline plus a
 * 2px accent caret; never a box, never a magnifier glyph. The blinking bar
 * shows while the line is empty; once text exists the native caret
 * (accent-coloured) takes over.
 */
export function SearchLine({ scale, value, onChange, placeholder, label }: SearchLineProps) {
  return (
    <div className={`tv-search-line tv-search-line-${scale}`}>
      {value === '' && <Caret height={CARET_HEIGHT[scale]} />}
      <input
        type="text"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

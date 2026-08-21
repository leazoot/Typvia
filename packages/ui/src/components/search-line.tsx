// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { ReactNode } from 'react';
import './search-line.css';

/** The three desktop scales; each maps to its type-scale role in tokens.json. */
export type SearchLineScale = 'hero' | 'library' | 'panel';

interface SearchLineProps {
  scale: SearchLineScale;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  /** Accessible name for the input (the search line has no visible label). */
  label: string;
  /** Optional right-aligned slot inside the underlined row (e.g. a count). */
  trailing?: ReactNode;
}

/**
 * The search line — the product's signature. A quiet hairline underline;
 * never a box, never a magnifier glyph. The native input caret
 * (accent-coloured) is the only cursor — no decorative bar doubles it.
 */
export function SearchLine({
  scale,
  value,
  onChange,
  placeholder,
  label,
  trailing,
}: SearchLineProps) {
  return (
    <div className={`tv-search-line tv-search-line-${scale}`}>
      <input
        type="text"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {trailing !== undefined && <span className="tv-search-line-trailing">{trailing}</span>}
    </div>
  );
}

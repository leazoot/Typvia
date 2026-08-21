// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { ReactNode } from 'react';
import './search-field.css';

interface SearchFieldProps {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  /** Accessible name for the input (the field has no visible label). */
  label: string;
  /** Optional right-aligned slot inside the field (e.g. a result count). */
  trailing?: ReactNode;
}

/**
 * The mobile search field: a real tappable rounded field at r12 — it
 * deliberately replaces the desktop underline search line, because the gesture
 * that line rehearses (⌘⇧V) does not exist on a phone. Still no box-in-box
 * input chrome, no magnifier glyph, no decorative caret — the native input
 * caret is the only cursor.
 */
export function SearchField({ value, onChange, placeholder, label, trailing }: SearchFieldProps) {
  return (
    <div className="tv-search-field">
      <input
        type="text"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {trailing !== undefined && <span className="tv-search-field-trailing">{trailing}</span>}
    </div>
  );
}

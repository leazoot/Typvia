// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useEffect, useRef } from 'react';
import { useTr } from '@typvia/ui';
import './search-line.css';

/**
 * "Search is a line, not a box": a vertical
 * caret bar, the text, and a hairline underline. Three appearances share the
 * skeleton — Home's entry button (accent bar, strong hairline), Library's
 * quiet inline filter (dimmed bar) and the Search screen's focused line
 * (accent underline, mono input, Cancel).
 */

interface SearchLineButtonProps {
  placeholder: string;
  label: string;
  onPress: () => void;
}

/** Home entry: looks like the live line, acts as a door to the Search screen. */
export function SearchLineButton({ placeholder, label, onPress }: SearchLineButtonProps) {
  return (
    <button type="button" className="tv-msline is-entry" aria-label={label} onClick={onPress}>
      <span className="tv-msline-bar" />
      <span className="tv-msline-placeholder">{placeholder}</span>
    </button>
  );
}

interface SearchLineInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  label: string;
  /** Library appearance: dimmed caret bar, mid hairline, no Cancel. */
  quiet?: boolean;
  autoFocus?: boolean;
  onCancel?: (() => void) | undefined;
}

/** A real input on the line; the native caret is accent-coloured. */
export function SearchLineInput({
  value,
  onChange,
  placeholder,
  label,
  quiet = false,
  autoFocus = false,
  onCancel,
}: SearchLineInputProps) {
  const tr = useTr();
  const inputRef = useRef<HTMLInputElement>(null);

  // autoFocus via effect: the attribute is unreliable inside a WebView that
  // is still animating the screen in.
  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, [autoFocus]);

  return (
    <div className={quiet ? 'tv-msline is-quiet' : 'tv-msline is-live'}>
      <span className="tv-msline-bar" />
      <input
        ref={inputRef}
        type="search"
        className="tv-msline-input"
        value={value}
        placeholder={placeholder}
        aria-label={label}
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {onCancel !== undefined && (
        <button type="button" className="tv-msline-cancel" onClick={onCancel}>
          {tr('Cancel', '取消')}
        </button>
      )}
    </div>
  );
}

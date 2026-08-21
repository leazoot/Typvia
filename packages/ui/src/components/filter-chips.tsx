// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import './filter-chips.css';

export interface FilterChip {
  key: string;
  label: string;
}

interface FilterChipRowProps {
  chips: readonly FilterChip[];
  activeKey: string;
  onSelect: (key: string) => void;
  /** Accessible name for the chip group. */
  label: string;
  /** Makes the whole row inert (announced, never hidden). */
  disabled?: boolean;
}

/**
 * Horizontally scrollable filter chip row for the Library.
 * The active chip is lifted — paper + hairline + e1 shadow on light, a
 * lightness lift on graphite — never a colour fill. Chips keep the 44pt
 * touch height.
 */
export function FilterChipRow({ chips, activeKey, onSelect, label, disabled }: FilterChipRowProps) {
  return (
    <div role="group" aria-label={label} className="tv-chip-row">
      {chips.map((chip) => {
        const active = chip.key === activeKey;
        return (
          <button
            key={chip.key}
            type="button"
            aria-pressed={active}
            aria-disabled={disabled === true || undefined}
            className={active ? 'tv-chip is-active' : 'tv-chip'}
            onClick={() => {
              if (disabled !== true) onSelect(chip.key);
            }}
          >
            {chip.label}
          </button>
        );
      })}
    </div>
  );
}

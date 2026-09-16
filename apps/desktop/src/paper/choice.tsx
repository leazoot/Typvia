// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useRef, type KeyboardEvent, type ReactNode } from 'react';

export interface ChoiceOption<T extends string> {
  value: T;
  label: string;
  /** Right-aligned detail on the option's line, such as a count. */
  trailing?: ReactNode;
}

const STEP: Readonly<Record<string, number>> = {
  ArrowRight: 1,
  ArrowDown: 1,
  ArrowLeft: -1,
  ArrowUp: -1,
};

/**
 * The coral trace option — the desktop's only single-choice control, standing
 * in for radios, segmented controls and tabs. Choosing takes effect at once:
 * no confirm, no save. Arrow keys move and choose; only the chosen option is
 * in the tab order, so a group is one Tab stop.
 */
export function Choice<T extends string>({
  label,
  options,
  value,
  onChange,
  stacked = false,
  marked = null,
}: {
  label: string;
  options: readonly ChoiceOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** One option per line (the collections column) instead of a row. */
  stacked?: boolean;
  /** An option to call out without choosing it, such as where a drag would land. */
  marked?: T | null;
}) {
  const group = useRef<HTMLDivElement>(null);
  const hasChosen = options.some((option) => option.value === value);

  const move = (event: KeyboardEvent, index: number) => {
    const step = STEP[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const nextIndex = (index + step + options.length) % options.length;
    const next = options[nextIndex];
    if (next === undefined) return;
    onChange(next.value);
    group.current?.querySelectorAll<HTMLElement>('[role="radio"]')[nextIndex]?.focus();
  };

  return (
    <div
      ref={group}
      role="radiogroup"
      aria-label={label}
      className={stacked ? 'tpi-choice is-stacked' : 'tpi-choice'}
    >
      {options.map((option, index) => {
        const chosen = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={chosen}
            tabIndex={chosen || (!hasChosen && index === 0) ? 0 : -1}
            className="tpi-choice-option"
            data-value={option.value}
            data-marked={marked === option.value ? true : undefined}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => move(event, index)}
          >
            <span className="tpi-choice-text">{option.label}</span>
            {option.trailing !== undefined && (
              <span className="tpi-choice-trailing">{option.trailing}</span>
            )}
            <span aria-hidden="true" className="tpi-choice-trace" />
          </button>
        );
      })}
    </div>
  );
}

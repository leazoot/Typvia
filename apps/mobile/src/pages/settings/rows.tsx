// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The mobile settings row vocabulary: a bordered group of 52pt rows, each
 * with a label column, an optional value and exactly one control — a
 * chevron, a toggle, or a word. No icons on mobile: every affordance here is
 * a shape or a word.
 *
 * Layout only. Everything these rows say comes from the shared sync layer.
 */
import type { ReactNode } from 'react';
import './settings.css';

interface GroupProps {
  /** Mono section label above the card, e.g. "This device". */
  label?: ReactNode;
  /** A footnote paragraph follows, so the group closes up against it. */
  annotated?: boolean;
  children: ReactNode;
}

export function SettingsGroup({ label, annotated = false, children }: GroupProps) {
  return (
    <>
      {label !== undefined && <div className="tv-mset-label">{label}</div>}
      <ul className={annotated ? 'tv-mset-group is-annotated' : 'tv-mset-group'}>{children}</ul>
    </>
  );
}

interface RowProps {
  label: ReactNode;
  /** Second line under the label. */
  sub?: ReactNode;
  /** Right-aligned reading of the row's current state. */
  value?: ReactNode;
  /** Status shape shown left of the value: healthy circle or warning square. */
  dot?: 'success' | 'warning';
  /** The whole row opens a pushed screen. */
  onPress?: () => void;
  /** Right-aligned control rendered instead of a chevron. */
  control?: ReactNode;
}

export function SettingsRow({ label, sub, value, dot, onPress, control }: RowProps) {
  const body = (
    <>
      <span className="tv-mset-row-text">
        <span className="tv-mset-row-label">{label}</span>
        {sub !== undefined && <span className="tv-mset-row-sub">{sub}</span>}
      </span>
      {(value !== undefined || dot !== undefined) && (
        <span className="tv-mset-row-value">
          {dot !== undefined && <span aria-hidden="true" className={`tv-mset-dot is-${dot}`} />}
          {value}
        </span>
      )}
      {control}
      {onPress !== undefined && (
        <span aria-hidden="true" className="tv-mset-chevron">
          ›
        </span>
      )}
    </>
  );

  if (onPress !== undefined) {
    return (
      <li>
        <button type="button" className="tv-mset-row" onClick={onPress}>
          {body}
        </button>
      </li>
    );
  }
  return <li className="tv-mset-row">{body}</li>;
}

interface ToggleProps {
  label: string;
  pressed: boolean;
  disabled: boolean;
  onToggle: () => void;
}

/** The one pill in the design system; the label carries the meaning. */
export function SettingsToggle({ label, pressed, disabled, onToggle }: ToggleProps) {
  return (
    <button
      type="button"
      className="tv-mset-toggle"
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onToggle}
    >
      <span aria-hidden="true" className="tv-mset-toggle-thumb" />
    </button>
  );
}

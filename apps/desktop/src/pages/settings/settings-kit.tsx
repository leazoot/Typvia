// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The few shapes the one-page settings is built from: a row with the label
 * column, an explanation that stays folded until asked for, and a field that
 * is a line under the words.
 */
import type { InputHTMLAttributes, ReactNode } from 'react';

export function SettingRow({
  label,
  children,
  narrow = false,
}: {
  label?: ReactNode;
  children: ReactNode;
  /** Caps the content width, for fields. */
  narrow?: boolean;
}) {
  return (
    <div className="tvs-row">
      <div className="tvs-label">{label}</div>
      <div className={narrow ? 'tvs-value is-narrow' : 'tvs-value'}>{children}</div>
    </div>
  );
}

/**
 * An explanation kept out of the way: folded until the pointer is over its row
 * or focus is inside it, then it opens. Screen readers always reach it.
 */
export function Hint({ children }: { children: ReactNode }) {
  return (
    <p className="tvs-hint">
      <span className="tvs-hint-text">{children}</span>
    </p>
  );
}

/** A line of words that answers straight away: done, or what stood in the way. */
export function Said({ children }: { children: ReactNode }) {
  return (
    <p className="tvs-said" role="status">
      {children}
    </p>
  );
}

export type StateKind = 'ok' | 'warn' | 'idle';

/** A 6px dot and a word: sage when ready, amber when something waits on you. */
export function State({ kind, children }: { kind: StateKind; children: ReactNode }) {
  return (
    <span className="tvs-state">
      <span aria-hidden="true" className={`tvs-state-dot is-${kind}`} />
      {children}
    </span>
  );
}

export function Actions({ children }: { children: ReactNode }) {
  return <div className="tvs-actions">{children}</div>;
}

/** Destructive words: quiet until hovered, then the one solid fill. */
export function DangerAction({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <button type="button" className="tvs-danger" onClick={onClick}>
      {children}
    </button>
  );
}

export function LineField({
  label,
  mono = false,
  ...input
}: InputHTMLAttributes<HTMLInputElement> & { label: string; mono?: boolean }) {
  return (
    <label className={mono ? 'tvs-line is-mono' : 'tvs-line'}>
      <span className="tvs-line-label">{label}</span>
      <input spellCheck={false} {...input} />
    </label>
  );
}

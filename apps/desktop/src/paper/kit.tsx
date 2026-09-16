// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The small parts of Paper & Ink that every surface speaks. Presentation only
 * — no IPC.
 */
import type { ReactNode } from 'react';
import './kit.css';

/** A group title: coral dot, spaced caps, and a hairline that fades out. */
export function GroupTitle({ children, trailing }: { children: ReactNode; trailing?: ReactNode }) {
  return (
    <div className="tpi-group">
      <span aria-hidden="true" className="tpi-group-dot" />
      <h2 className="tpi-group-label">{children}</h2>
      <span aria-hidden="true" className="tpi-group-line" />
      {trailing}
    </div>
  );
}

export function KeyCap({ children }: { children: ReactNode }) {
  return <kbd className="tpi-key">{children}</kbd>;
}

/**
 * Every action is words, never a filled button. The primary one is coral and
 * carries an arrow; on hover only the gap opens, from 11px to 16px.
 */
export function TextAction({
  children,
  onClick,
  primary = false,
  disabled = false,
}: {
  children: ReactNode;
  onClick: () => void;
  primary?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className={primary ? 'tpi-action is-primary' : 'tpi-action'}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
      {primary && (
        <svg width="28" height="11" viewBox="0 0 34 11" fill="none" aria-hidden="true">
          <path
            d="M0 5.5h30M25 1l5 4.5-5 4.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )}
    </button>
  );
}

/** The check box: a 17px square that fills coral; the label is part of the target. */
export function CheckBox({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children: ReactNode;
}) {
  return (
    <label className="tpi-check">
      <input
        type="checkbox"
        className="tpi-check-input"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span aria-hidden="true" className="tpi-check-box">
        <svg width="10" height="8" viewBox="0 0 10 8" fill="none">
          <path
            d="M1 4l2.6 2.6L9 1.2"
            stroke="currentColor"
            strokeWidth="1.9"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </span>
      <span className="tpi-check-label">{children}</span>
    </label>
  );
}

/** A note pinned slightly askew: how the product speaks up without a modal. */
export function Note({ children, label }: { children: ReactNode; label: string }) {
  return (
    <div className="tpi-note" role="status" aria-label={label}>
      {children}
    </div>
  );
}

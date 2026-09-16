// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useId } from 'react';
import './mascot.css';

export type MascotState =
  'idle' | 'happy' | 'typing' | 'thinking' | 'confused' | 'sleep' | 'locked';

const STEM = { fill: 'var(--accent)' };
const FACE_FILL = { fill: 'var(--accent-face)' };
const FACE_LINE = { stroke: 'var(--accent-face)' };

function Eyes({ state }: { state: MascotState }) {
  if (state === 'happy') {
    return (
      <g fill="none" strokeWidth="2.1" strokeLinecap="round" style={FACE_LINE}>
        <path d="M14.5 20.2Q16.6 17.3 18.7 20.2" />
        <path d="M25.3 20.2Q27.4 17.3 29.5 20.2" />
      </g>
    );
  }
  if (state === 'sleep' || state === 'locked') {
    return (
      <g strokeWidth="2.1" strokeLinecap="round" style={FACE_LINE}>
        <path d="M14.6 19.6h4" />
        <path d="M25.4 19.6h4" />
      </g>
    );
  }
  if (state === 'confused') {
    return (
      <g>
        <path d="M14.8 19.6h3.6" strokeWidth="2.1" strokeLinecap="round" style={FACE_LINE} />
        <circle cx="27.4" cy="19.2" r="2.4" style={FACE_FILL} />
      </g>
    );
  }
  // Thinking lifts the eyes, as if looking up at the dots.
  const y = state === 'thinking' ? 18.4 : 19.6;
  return (
    <g style={FACE_FILL}>
      <circle cx="16.6" cy={y} r="2.4" />
      <circle cx="27.4" cy={y} r="2.4" />
    </g>
  );
}

function Mouth({ state }: { state: MascotState }) {
  switch (state) {
    case 'happy':
      return (
        <path
          d="M16 23.8Q22 29.8 28 23.8"
          fill="none"
          strokeWidth="2.2"
          strokeLinecap="round"
          style={FACE_LINE}
        />
      );
    case 'sleep':
      return <path d="M19.4 25.8h5.2" strokeWidth="2.1" strokeLinecap="round" style={FACE_LINE} />;
    case 'confused':
      return (
        <path
          d="M18.2 26q1.9-2.2 3.8 0t3.8 0"
          fill="none"
          strokeWidth="2"
          strokeLinecap="round"
          style={FACE_LINE}
        />
      );
    case 'thinking':
      return (
        <ellipse
          cx="22"
          cy="25.6"
          rx="1.7"
          ry="1.9"
          fill="none"
          strokeWidth="1.9"
          style={FACE_LINE}
        />
      );
    case 'typing':
      return (
        <g>
          <path d="M18.4 25.6h4" strokeWidth="2.1" strokeLinecap="round" style={FACE_LINE} />
          <rect
            className="tpi-mascot-blink"
            x="24.6"
            y="22.8"
            width="1.8"
            height="5.6"
            rx="0.9"
            style={FACE_FILL}
          />
        </g>
      );
    default:
      return (
        <path
          d="M16.8 24.4Q22 28.4 27.2 24.4"
          fill="none"
          strokeWidth="2.1"
          strokeLinecap="round"
          style={FACE_LINE}
        />
      );
  }
}

/**
 * The mascot: a speech bubble whose antenna is the product's caret. It is a
 * state indicator, not decoration, and it always stands next to words that
 * already say the state — so it stays out of the accessibility tree.
 */
export function Mascot({ state = 'idle', size }: { state?: MascotState; size: number }) {
  // useId output carries characters an SVG url() reference cannot hold.
  const gradient = `tpi-mascot-${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const body = { fill: `url(#${gradient})` };
  return (
    <svg
      className="tpi-mascot"
      data-state={state}
      viewBox="0 0 44 44"
      width={size}
      height={size}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={gradient} gradientUnits="userSpaceOnUse" x1="8" y1="5" x2="36" y2="42">
          <stop offset="0" style={{ stopColor: 'var(--accent-light)' }} />
          <stop offset="1" style={{ stopColor: 'var(--accent)' }} />
        </linearGradient>
      </defs>
      {state === 'locked' ? (
        <path
          d="M15.5 8a6.5 6.5 0 0113 0"
          fill="none"
          strokeWidth="2.6"
          strokeLinecap="round"
          style={{ stroke: 'var(--accent)' }}
        />
      ) : (
        <rect
          className={state === 'typing' ? 'tpi-mascot-blink' : undefined}
          x="20.9"
          y="0"
          width="2.2"
          height="8.5"
          rx="1.1"
          style={STEM}
        />
      )}
      {state === 'thinking' && (
        <g style={STEM}>
          <circle className="tpi-mascot-dot" cx="5.6" cy="9.4" r="1.9" />
          <circle className="tpi-mascot-dot is-second" cx="10.2" cy="5" r="1.5" />
          <circle className="tpi-mascot-dot is-third" cx="14.6" cy="1.9" r="1.1" />
        </g>
      )}
      {state === 'happy' && (
        <path
          className="tpi-mascot-sparkle"
          d="M36.8 1.6v6M33.8 4.6h6"
          strokeWidth="1.7"
          strokeLinecap="round"
          style={{ stroke: 'var(--accent)' }}
        />
      )}
      {state === 'sleep' && (
        <g
          className="tpi-mascot-z"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
          style={{ stroke: 'var(--accent)' }}
        >
          <path d="M32.6 6.4h4l-4 4h4" strokeWidth="1.5" />
          <path d="M38.8 1.2h2.8l-2.8 2.8h2.8" strokeWidth="1.3" opacity="0.7" />
        </g>
      )}
      <path
        d="M18.4 33.2C18.6 37.3 18.1 39.9 16.8 41.6C16.4 42.1 16.9 42.7 17.5 42.4C20.8 40.9 23.5 37.8 24.9 34.2Z"
        style={body}
      />
      <ellipse cx="22" cy="21" rx="17" ry="15" style={body} />
      <Eyes state={state} />
      <Mouth state={state} />
    </svg>
  );
}

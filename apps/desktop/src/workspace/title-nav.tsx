// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr, type Tr } from '@typvia/ui';
import { useLocation, useNavigate } from 'react-router';
import './title-nav.css';

interface Place {
  path: string;
  label: (tr: Tr) => string;
  /** Pages that count as being in this place, so its mark stays lit there. */
  roots: readonly string[];
}

const PLACES: readonly Place[] = [
  { path: '/', label: (tr) => tr('Snippets', '片段'), roots: ['/editor'] },
  { path: '/vault', label: (tr) => tr('Vault', '保险库'), roots: ['/vault'] },
  { path: '/trash', label: (tr) => tr('Trash', '回收站'), roots: ['/trash'] },
  {
    path: '/settings',
    label: (tr) => tr('Settings', '设置'),
    roots: ['/settings', '/sync', '/ai', '/templates', '/shortcuts'],
  },
];

function isIn(place: Place, pathname: string): boolean {
  if (pathname === place.path) return true;
  return place.roots.some((root) => pathname === root || pathname.startsWith(`${root}/`));
}

/**
 * The four places as words in the title bar; a click goes straight there. The
 * place the window is in carries a short coral wave under its name.
 */
export function PlaceTabs() {
  const tr = useTr();
  const { pathname } = useLocation();
  const navigate = useNavigate();
  return (
    <nav className="tvtn-tabs" aria-label={tr('Places', '去处')}>
      {PLACES.map((place) => {
        const here = isIn(place, pathname);
        return (
          <button
            key={place.path}
            type="button"
            className="tvtn-tab"
            aria-current={here ? 'page' : undefined}
            onClick={() => {
              if (pathname !== place.path) void navigate(place.path);
            }}
          >
            {place.label(tr)}
            {here && <Wave />}
          </button>
        );
      })}
    </nav>
  );
}

/** Only one place is lit at a time, so one gradient id is enough. */
const WAVE_GRADIENT = 'tvtn-wave-ink';

function Wave() {
  return (
    <svg className="tvtn-wave" viewBox="0 0 40 6" preserveAspectRatio="none" aria-hidden="true">
      <defs>
        <linearGradient id={WAVE_GRADIENT} x1="0" x2="1" y1="0" y2="0">
          <stop offset="0" style={{ stopColor: 'var(--accent-light)' }} />
          <stop offset="1" style={{ stopColor: 'var(--accent)' }} />
        </linearGradient>
      </defs>
      <path
        d="M1 3.4C7 0.9 13.5 0.9 20 3.4S33 5.9 39 3.4"
        fill="none"
        stroke={`url(#${WAVE_GRADIENT})`}
        strokeWidth="2"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

/**
 * Steps back to where the window was before; opened straight onto a page with
 * nothing behind it, it goes to the library. Absent on the library itself.
 */
export function BackAction() {
  const tr = useTr();
  const location = useLocation();
  const navigate = useNavigate();
  if (location.pathname === '/') return null;
  // React Router names the entry the window started on "default".
  const hasPast = location.key !== 'default';
  return (
    <button
      type="button"
      className="tvtn-back"
      onClick={() => void (hasPast ? navigate(-1) : navigate('/'))}
    >
      <svg width="13" height="10" viewBox="0 0 13 10" fill="none" aria-hidden="true">
        <path
          d="M12 5H1.5M5 1.2L1.2 5 5 8.8"
          stroke="currentColor"
          strokeWidth="1.3"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      {tr('Back', '返回')}
    </button>
  );
}

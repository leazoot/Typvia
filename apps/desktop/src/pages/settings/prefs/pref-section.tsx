// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The peek/fold shell of the preferences surface: an accordion of summary
 * rows. One section opens at a time; ⌥-click keeps others open (the desktop
 * detail from the design). While anything is open, closed rows lose weight.
 */
import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react';
import { Glyph, type GlyphName } from './pref-kit';

interface PrefsState {
  openKeys: ReadonlySet<string>;
  toggle: (key: string, keepOthers: boolean) => void;
  openOnly: (key: string) => void;
}

const PrefsContext = createContext<PrefsState>({
  openKeys: new Set(),
  toggle: () => undefined,
  openOnly: () => undefined,
});

export function usePrefs(): PrefsState {
  return useContext(PrefsContext);
}

export function PrefsProvider({ children }: { children: ReactNode }) {
  const [openKeys, setOpenKeys] = useState<ReadonlySet<string>>(new Set());

  const toggle = useCallback((key: string, keepOthers: boolean) => {
    setOpenKeys((current) => {
      const next = new Set(keepOthers ? current : current.has(key) ? [key] : []);
      if (current.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const openOnly = useCallback((key: string) => {
    setOpenKeys(new Set([key]));
  }, []);

  const value = useMemo(() => ({ openKeys, toggle, openOnly }), [openKeys, toggle, openOnly]);
  return <PrefsContext.Provider value={value}>{children}</PrefsContext.Provider>;
}

export function PrefSections({ children }: { children: ReactNode }) {
  const { openKeys } = usePrefs();
  return <div className={openKeys.size > 0 ? 'tvp-dimmed' : ''}>{children}</div>;
}

export function PrefSection({
  id,
  glyph,
  name,
  sub,
  status,
  actions,
  children,
}: {
  id: string;
  glyph: GlyphName;
  name: ReactNode;
  /** One quiet summary line under the name. */
  sub: ReactNode;
  /** Right-aligned state: a dot and one word, or nothing. */
  status?: ReactNode;
  /** Contextual actions revealed on hover of the summary row. */
  actions?: ReactNode;
  children: ReactNode;
}) {
  const { openKeys, toggle } = usePrefs();
  const open = openKeys.has(id);

  return (
    <section className={`tvp-pref ${open ? 'open' : ''}`} data-pref={id}>
      <button
        type="button"
        className="tvp-row"
        aria-expanded={open}
        onClick={(event) => toggle(id, event.altKey)}
      >
        <Glyph name={glyph} />
        <span className="tvp-row-main">
          <span className="tvp-row-name">{name}</span>
          <span className="tvp-row-sub">{sub}</span>
        </span>
        <span className="tvp-row-status">{status}</span>
        <span className="tvp-row-actions">{actions}</span>
        <span aria-hidden="true" className="tvp-chev">
          ›
        </span>
      </button>
      <div className="tvp-panel">
        <div className="tvp-panel-clip">
          <div className="tvp-panel-inner">{open ? children : null}</div>
        </div>
      </div>
    </section>
  );
}

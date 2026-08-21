// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Typvia context navigation: Home, the current workspace
 * room behind a switcher, and Settings. Three text destinations, no sidebar,
 * no pills — the active one is marked by weight plus a 2px indicator.
 */
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { Glyph } from './kit';
import { ROOMS, roomFor } from './rooms';

interface WorkspaceNavProps {
  pathname: string;
  onNavigate: (path: string) => void;
}

export function WorkspaceNav({ pathname, onNavigate }: WorkspaceNavProps) {
  const tr = useTr();
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  const room = roomFor(pathname);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (!wrap.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', close);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);

  const go = (path: string) => {
    setOpen(false);
    onNavigate(path);
  };

  const switcherLabel = room ? tr(room.labelEn, room.labelZh) : tr('Workspace', '工作台');
  const settingsActive = pathname === '/settings';

  return (
    <nav aria-label={tr('Main', '主导航')} className="tvw-nav">
      <button
        type="button"
        className="tvw-nav-item"
        aria-current={pathname === '/' ? 'page' : undefined}
        onClick={() => onNavigate('/')}
      >
        <span className="tvw-nav-text" data-label={tr('Home', '首页')}>
          {tr('Home', '首页')}
        </span>
      </button>

      <span ref={wrap} className="tvw-pop-wrap">
        <button
          type="button"
          className={room ? 'tvw-nav-item is-active' : 'tvw-nav-item'}
          aria-expanded={open}
          aria-haspopup="menu"
          onClick={() => setOpen((current) => !current)}
        >
          <span className="tvw-nav-text" data-label={switcherLabel}>
            {switcherLabel}
          </span>
          <span aria-hidden="true" className="tvw-nav-caret">
            ▾
          </span>
        </button>
        {open && (
          <span className="tvw-pop" role="menu" aria-label={tr('Workspace', '工作台')}>
            <span className="tvw-pop-label">{tr('Workspace', '工作台')}</span>
            {ROOMS.map((entry) => {
              const here = entry.path === room?.path;
              return (
                <button
                  key={entry.path}
                  type="button"
                  role="menuitem"
                  className="tvw-pop-item"
                  onClick={() => go(entry.path)}
                >
                  <span className="tvw-mark" aria-hidden="true">
                    {entry.mark}
                  </span>
                  <span>
                    <span className="tvw-pop-name">{tr(entry.labelEn, entry.labelZh)}</span>
                    <span className="tvw-pop-desc">{tr(entry.descEn, entry.descZh)}</span>
                  </span>
                  {here ? (
                    <span className="tvw-pop-here" role="img" aria-label={tr('Current', '当前')} />
                  ) : (
                    <span aria-hidden="true" className="tvw-nav-shortcut">
                      ⌘{entry.digit}
                    </span>
                  )}
                </button>
              );
            })}
          </span>
        )}
      </span>

      {/* Settings is a glyph that morphs into its word on hover and while it
          is the active destination. */}
      <button
        type="button"
        className={settingsActive ? 'tvw-nav-item is-active' : 'tvw-nav-item'}
        aria-current={settingsActive ? 'page' : undefined}
        aria-label={tr('Settings', '设置')}
        onClick={() => onNavigate('/settings')}
      >
        <Glyph name="gear" />
        <span aria-hidden="true" className="tvw-nav-morph">
          {tr('Settings', '设置')}
          <span className="tvw-nav-shortcut" style={{ marginLeft: 6 }}>
            ⌘,
          </span>
        </span>
      </button>
    </nav>
  );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The desktop workspace shell: one 56px header over the routed page, the
 * command palette, and the keyboard map that makes every room reachable
 * without the mouse.
 */
import { useTr } from '@typvia/ui';
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { WorkspaceNav } from './nav';
import { CommandPalette, type PaletteEntry } from './palette';
import { ROOMS, roomFor } from './rooms';
import './workspace.css';

/** Which room the canvas is in — the vault gets its quieter surface. */
function surfaceFor(pathname: string): string {
  if (pathname === '/') return 'home';
  if (pathname === '/settings') return 'settings';
  return roomFor(pathname)?.path.slice(1) ?? 'page';
}

export function WorkspaceShell({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const location = useLocation();
  const tr = useTr();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const go = (path: string) => void navigate(path);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setPaletteOpen(false);
        return;
      }
      if (!event.metaKey && !event.ctrlKey) return;
      const key = event.key.toLowerCase();
      if (key === 'k') {
        event.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (key === ',') {
        event.preventDefault();
        void navigate('/settings');
        return;
      }
      if (key === 'n') {
        event.preventDefault();
        void navigate('/editor');
        return;
      }
      const room = ROOMS.find((entry) => String(entry.digit) === event.key);
      if (room !== undefined) {
        event.preventDefault();
        void navigate(room.path);
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [navigate]);

  const entries = useMemo<PaletteEntry[]>(() => {
    const navigateGroup = tr('Navigate', '前往');
    const rooms: PaletteEntry[] = ROOMS.map((room) => ({
      id: room.path,
      group: navigateGroup,
      label: tr(room.labelEn, room.labelZh),
      shortcut: `⌘${String(room.digit)}`,
      run: () => void navigate(room.path),
    }));
    return [
      {
        id: '/',
        group: navigateGroup,
        label: tr('Home', '首页'),
        run: () => void navigate('/'),
      },
      ...rooms,
      {
        // Not one of the four rooms, but the palette keeps it reachable
        // — the template builder is opened from a template, or from here.
        id: '/templates',
        group: navigateGroup,
        label: tr('Template builder', '模板编辑器'),
        run: () => void navigate('/templates'),
      },
      {
        id: '/settings',
        group: navigateGroup,
        label: tr('Settings', '设置'),
        shortcut: '⌘,',
        run: () => void navigate('/settings'),
      },
      {
        id: 'new-snippet',
        group: tr('Actions', '动作'),
        label: tr('New snippet', '新建片段'),
        shortcut: '⌘N',
        run: () => void navigate('/editor'),
      },
    ];
  }, [navigate, tr]);

  return (
    <div className="tvw-shell">
      <header className="tvw-header" data-tauri-drag-region="">
        <WorkspaceNav pathname={location.pathname} onNavigate={go} />
      </header>
      <main
        key={location.pathname}
        className="tvw-main tvw-enter"
        data-surface={surfaceFor(location.pathname)}
      >
        {children}
      </main>
      {paletteOpen && <CommandPalette entries={entries} onClose={() => setPaletteOpen(false)} />}
    </div>
  );
}

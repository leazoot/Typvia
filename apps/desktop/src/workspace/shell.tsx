// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { listen } from '@tauri-apps/api/event';

/**
 * The desktop window shell: a 38px glass title bar with the places and the way
 * back over the routed page, the macOS menu bar with its status lines, the
 * command palette, the undo note, and the keyboard map that makes every room
 * reachable without the mouse.
 */
import {
  aboutShow,
  clipboardReadText,
  listTrash,
  syncNow,
  syncStatus,
  vaultList,
} from '@typvia/shared';
import type { Folder } from '@typvia/shared';
import { trFor, useLocale, useTr, type Tr } from '@typvia/ui';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { fetchFolders } from '../pages/library/library-data';
import { trashSummary } from '../pages/trash/trash-model';
import { KeyCap } from '../paper/kit';
import { useVault } from '../vault/vault-context';
import { installAppMenu } from './app-menu';
import { commandState, runCommand } from './commands';
import { announceLibraryChange, useLibraryChange } from './library-events';
import type { MenuAction, MenuStatus } from './menu-model';
import { CommandPalette, type PaletteEntry } from './palette';
import { ROOMS, roomFor } from './rooms';
import { TitleBarContext, type TitleBarContent } from './title-bar';
import { BackAction, PlaceTabs } from './title-nav';
import { UndoProvider, useUndo } from './undo';
import { WindowBar } from './window-bar';
import './shell.css';

/** The menu bar is a macOS surface; elsewhere the window will carry its own. */
const HAS_MENU_BAR = typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac');
/** Windows has no menu bar and no title bar of ours from the system: the page draws both. */
const DRAWS_WINDOW_BAR =
  typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');
/** The menu-bar / tray card exists only inside the desktop host on these two platforms. */
const HAS_TRAY = HAS_MENU_BAR || DRAWS_WINDOW_BAR;
/** The vault line counts down in minutes; this is often enough to keep it true. */
const MENU_TICK_MS = 30_000;

function opens(action: MenuAction): action is `open:${string}` {
  return action.startsWith('open:');
}

function moves(action: MenuAction): action is `snippet.move:${string}` {
  return action.startsWith('snippet.move:');
}

/** Which room the canvas is in — the vault gets its quieter surface. */
function surfaceFor(pathname: string): string {
  if (pathname === '/') return 'library';
  if (pathname === '/settings') return 'settings';
  if (pathname.startsWith('/editor')) return 'editor';
  return roomFor(pathname)?.path.slice(1) ?? 'page';
}

/** The facts the menu bar's rows and closing lines are drawn from. */
function useMenuStatus(route: string, tr: Tr): MenuStatus {
  const { status: vault } = useVault();
  const { canUndo } = useUndo();
  const [vaultCount, setVaultCount] = useState<number | null>(null);
  const [trash, setTrash] = useState<{ count: number; soonest: number | null }>({
    count: 0,
    soonest: null,
  });
  const [syncOn, setSyncOn] = useState(false);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [now, setNow] = useState(() => Date.now());

  const read = useCallback(() => {
    if (!HAS_MENU_BAR) return;
    vaultList(500, 0)
      .then((rows) => setVaultCount(rows.length))
      .catch(() => setVaultCount(null));
    listTrash(500, 0)
      .then((rows) => setTrash(trashSummary(rows, Date.now())))
      .catch(() => setTrash({ count: 0, soonest: null }));
    syncStatus()
      .then((status) => setSyncOn(status.enabled))
      .catch(() => setSyncOn(false));
    fetchFolders()
      .then(setFolders)
      .catch(() => setFolders([]));
  }, []);
  useEffect(read, [read, vault?.unlocked]);
  useLibraryChange(read);
  useEffect(() => {
    if (!HAS_MENU_BAR) return;
    const timer = setInterval(() => setNow(Date.now()), MENU_TICK_MS);
    return () => clearInterval(timer);
  }, []);

  const n = vaultCount === null ? null : String(vaultCount);
  let vaultLine: string;
  if (vault === null) vaultLine = tr('Vault', '保险库');
  else if (!vault.initialized) vaultLine = tr('No vault yet', '还没建保险库');
  else if (!vault.unlocked) {
    vaultLine = n === null ? tr('Locked', '已锁') : tr(`${n} · locked`, `${n} 条 · 已锁`);
  } else {
    const base = vault.lastActivityAt ?? vault.unlockedAt ?? now;
    const minutes = String(Math.max(1, Math.ceil((base + vault.idleTimeoutMs - now) / 60_000)));
    vaultLine = tr(
      `${n ?? '—'} · locks in ${minutes} min`,
      `${n ?? '—'} 条 · ${minutes} 分钟后重锁`,
    );
  }
  const count = String(trash.count);
  const soonest = String(trash.soonest ?? 0);
  const trashLine =
    trash.count === 0
      ? tr('The trash is empty', '回收站是空的')
      : tr(
          `${count} · the oldest has ${soonest} days left`,
          `${count} 条 · 最早那条还剩 ${soonest} 天`,
        );

  return {
    route,
    vaultUnlocked: vault?.unlocked === true,
    vaultLine,
    trashCount: trash.count,
    trashLine,
    canUndo,
    syncOn,
    collections: [
      ...folders.map((folder) => ({ id: folder.id, name: folder.name })),
      { id: '', name: tr('Unsorted', '待整理') },
    ],
  };
}

export function WorkspaceShell({ children }: { children: ReactNode }) {
  return (
    <UndoProvider>
      <ShellFrame>{children}</ShellFrame>
    </UndoProvider>
  );
}

function ShellFrame({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const location = useLocation();
  const locale = useLocale();
  const tr = useTr();
  const vault = useVault();
  const { undoLast, say } = useUndo();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [titleBar, setTitleBar] = useState<TitleBarContent | null>(null);

  // "Open the library" in the tray card lands here.
  useEffect(() => {
    if (!HAS_TRAY) return;
    const pending = listen('tray:library', () => void navigate('/'));
    return () => {
      void pending.then((unlisten) => {
        unlisten();
      });
    };
  }, [navigate]);

  const newCollection = useCallback(() => {
    if (!runCommand('collection.new'))
      void navigate('/', { state: commandState('collection.new') });
  }, [navigate]);

  const act = (action: MenuAction) => {
    if (opens(action)) {
      void navigate(action.slice('open:'.length));
      return;
    }
    if (moves(action)) {
      runCommand('snippet.move', action.slice('snippet.move:'.length));
      return;
    }
    switch (action) {
      case 'snippet.new':
        void navigate('/editor');
        return;
      case 'snippet.from-clipboard':
        clipboardReadText()
          .then((text) => void navigate('/editor', { state: { draftBody: text ?? '' } }))
          .catch(() =>
            say({
              title: tr("The clipboard couldn't be read.", '剪贴板没读出来。'),
              body: tr(
                'Nothing was created; copy it again and retry.',
                '什么都没新建;再复制一次试试。',
              ),
            }),
          );
        return;
      case 'vault.lock':
        void vault.lock();
        return;
      case 'about.open':
        aboutShow().catch(() =>
          say({
            title: tr("About didn't open.", '关于窗口没打开。'),
            body: tr('Nothing else is affected.', '别的都不受影响。'),
          }),
        );
        return;
      case 'trash.undo':
        undoLast();
        return;
      case 'sync.now':
        syncNow()
          .then(announceLibraryChange)
          .catch(() =>
            say({
              title: tr("Didn't sync this time.", '这次没同步上。'),
              body: tr(
                'Everything is still on this Mac; it tries again by itself.',
                '东西都还在这台 Mac 上,稍后会自己再试。',
              ),
            }),
          );
        return;
      case 'collection.new':
        newCollection();
        return;
      case 'trash.empty':
        if (!runCommand('trash.empty'))
          void navigate('/trash', { state: commandState('trash.empty') });
        return;
      default:
        // Rows acting on "the picked one" are enabled only where there is a pick.
        runCommand(action);
    }
  };
  const latestAct = useRef(act);
  latestAct.current = act;

  const menuStatus = useMenuStatus(location.pathname, tr);
  const updateMenu = useRef<((status: MenuStatus) => Promise<void>) | null>(null);
  const [menuVersion, setMenuVersion] = useState(0);

  useEffect(() => {
    if (!HAS_MENU_BAR) return;
    let live = true;
    installAppMenu(trFor(locale), (action) => latestAct.current(action))
      .then((update) => {
        if (!live) return;
        updateMenu.current = update;
        setMenuVersion((n) => n + 1);
      })
      .catch(() => {
        // The system's default menu stays up; every destination is still one
        // shortcut or one palette entry away.
      });
    return () => {
      live = false;
    };
  }, [locale]);

  const statusKey = JSON.stringify(menuStatus);
  const latestStatus = useRef(menuStatus);
  latestStatus.current = menuStatus;
  useEffect(() => {
    updateMenu.current?.(latestStatus.current).catch(() => {
      // A row left stale says an old number until the next change rewrites it.
    });
  }, [statusKey, menuVersion]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setPaletteOpen(false);
        return;
      }
      if (!event.metaKey && !event.ctrlKey) return;
      const key = event.key.toLowerCase();
      if (key === 'k' && event.shiftKey) {
        event.preventDefault();
        newCollection();
        return;
      }
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
      if (key === 'l' && !event.shiftKey && !event.altKey) {
        event.preventDefault();
        void vault.lock();
        return;
      }
      if (key === 'backspace' && event.shiftKey) {
        event.preventDefault();
        void navigate('/trash');
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
  }, [navigate, newCollection, vault]);

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
        id: '/sync',
        group: navigateGroup,
        label: tr('Sync & devices', '同步与设备'),
        run: () => void navigate('/sync'),
      },
      {
        id: '/shortcuts',
        group: navigateGroup,
        label: tr('All shortcuts', '快捷键总览'),
        run: () => void navigate('/shortcuts'),
      },
      {
        id: 'about',
        group: navigateGroup,
        label: tr('About Typvia', '关于 Typvia'),
        run: () => latestAct.current('about.open'),
      },
      {
        id: 'new-snippet',
        group: tr('Actions', '动作'),
        label: tr('New snippet', '新建片段'),
        shortcut: '⌘⇧N',
        run: () => void navigate('/editor'),
      },
      {
        id: 'new-from-clipboard',
        group: tr('Actions', '动作'),
        label: tr('New from the clipboard', '用剪贴板里的内容新建'),
        run: () => latestAct.current('snippet.from-clipboard'),
      },
      {
        id: 'new-collection',
        group: tr('Actions', '动作'),
        label: tr('New collection', '新建集合'),
        shortcut: '⌘⇧K',
        run: newCollection,
      },
    ];
  }, [navigate, newCollection, tr]);

  const surface = surfaceFor(location.pathname);
  return (
    <TitleBarContext.Provider value={setTitleBar}>
      <div
        className="tvw-shell"
        data-surface={surface}
        data-chrome={DRAWS_WINDOW_BAR ? 'windows' : 'mac'}
      >
        {DRAWS_WINDOW_BAR ? (
          <WindowBar />
        ) : (
          <header className="tpi tpi-titlebar" data-tauri-drag-region="">
            <span className="tpi-titlebar-lead" data-tauri-drag-region="">
              <PlaceTabs />
              <BackAction />
            </span>
            <span className="tpi-titlebar-summon">
              {titleBar?.trailing ?? (
                <>
                  <span>{tr('Summon', '呼出')}</span>
                  <KeyCap>⌘⇧V</KeyCap>
                </>
              )}
            </span>
          </header>
        )}
        <main key={location.pathname} className="tvw-main tvw-enter" data-surface={surface}>
          {children}
        </main>
        {paletteOpen && <CommandPalette entries={entries} onClose={() => setPaletteOpen(false)} />}
      </div>
    </TitleBarContext.Provider>
  );
}

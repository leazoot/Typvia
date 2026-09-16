// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { Menu, MenuItem, PredefinedMenuItem, Submenu } from '@tauri-apps/api/menu';
import type { Tr } from '@typvia/ui';
import { menuSections, type MenuAction, type MenuStatus } from './menu-model';

type Update = (status: MenuStatus) => Promise<void>;

/**
 * The macOS menu bar: the app menu, then the four menus drawn from the shared
 * rows, then the system Edit menu, without which cut / copy / paste never
 * reach a field. Returns the function that brings enabled rows and closing
 * lines up to date.
 */
export async function installAppMenu(tr: Tr, act: (action: MenuAction) => void): Promise<Update> {
  const nativeRow = (text: string, action: MenuAction, accelerator?: string) =>
    MenuItem.new({
      text,
      action: () => act(action),
      ...(accelerator === undefined ? {} : { accelerator }),
    });
  const separator = () => PredefinedMenuItem.new({ item: 'Separator' });

  const appMenu = await Submenu.new({
    text: 'Typvia',
    items: [
      await nativeRow(tr('About Typvia', '关于 Typvia'), 'about.open'),
      await separator(),
      await nativeRow(tr('Settings…', '设置…'), 'open:/settings', 'CmdOrCtrl+,'),
      await separator(),
      await PredefinedMenuItem.new({ item: 'Services' }),
      await separator(),
      await PredefinedMenuItem.new({ item: 'Hide', text: tr('Hide Typvia', '隐藏 Typvia') }),
      await PredefinedMenuItem.new({ item: 'HideOthers' }),
      await PredefinedMenuItem.new({ item: 'ShowAll' }),
      await separator(),
      await PredefinedMenuItem.new({ item: 'Quit', text: tr('Quit Typvia', '退出 Typvia') }),
    ],
  });

  const updates: Update[] = [];
  const sections: Submenu[] = [];
  for (const section of menuSections(tr)) {
    const items: Array<MenuItem | PredefinedMenuItem | Submenu> = [];
    for (const part of section.parts) {
      if (part.kind === 'separator') {
        items.push(await separator());
      } else if (part.kind === 'line') {
        const item = await MenuItem.new({ text: '', enabled: false });
        updates.push((status) => item.setText(part.text(status)));
        items.push(item);
      } else if (part.kind === 'collections') {
        const submenu = await Submenu.new({ text: part.label, items: [] });
        let shown: string | null = null;
        updates.push(async (status) => {
          await submenu.setEnabled(part.enabled(status));
          const key = status.collections.map((c) => `${c.id} ${c.name}`).join('');
          if (key === shown) return;
          shown = key;
          const previous = await submenu.items();
          for (let index = previous.length - 1; index >= 0; index -= 1) {
            await submenu.removeAt(index);
          }
          await submenu.append(
            await Promise.all(
              status.collections.map((collection) =>
                nativeRow(collection.name, `snippet.move:${collection.id}`),
              ),
            ),
          );
        });
        items.push(submenu);
      } else {
        const item = await nativeRow(part.label, part.action, part.accelerator);
        const enabled = part.enabled;
        if (enabled !== undefined) updates.push((status) => item.setEnabled(enabled(status)));
        items.push(item);
      }
    }
    sections.push(await Submenu.new({ text: section.title, items }));
  }

  const editMenu = await Submenu.new({
    text: tr('Edit', '编辑'),
    items: [
      await PredefinedMenuItem.new({ item: 'Undo' }),
      await PredefinedMenuItem.new({ item: 'Redo' }),
      await separator(),
      await PredefinedMenuItem.new({ item: 'Cut' }),
      await PredefinedMenuItem.new({ item: 'Copy' }),
      await PredefinedMenuItem.new({ item: 'Paste' }),
      await PredefinedMenuItem.new({ item: 'SelectAll' }),
    ],
  });

  const menu = await Menu.new({ items: [appMenu, ...sections, editMenu] });
  await menu.setAsAppMenu();

  return async (status) => {
    await Promise.all(updates.map((update) => update(status)));
  };
}

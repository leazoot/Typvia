// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { I18nProvider } from '../i18n';
import { FilterChipRow } from './filter-chips';
import { SearchField } from './search-field';
import { SnippetStrip, SnippetStripSkeleton } from './snippet-strip';
import { TabBar } from './tab-bar';
import { markForType } from './type-mark';

afterEach(cleanup);

const TAB_ITEMS = [
  { key: 'home', label: 'Home', icon: <svg data-testid="home-icon" /> },
  { key: 'library', label: 'Library' },
  { key: 'vault', label: 'Vault', disabled: true },
];

describe('TabBar', () => {
  it('renders a line icon above the label when the item carries one', () => {
    render(<TabBar items={TAB_ITEMS} activeKey="home" onSelect={() => {}} />);
    expect(screen.getByTestId('home-icon')).toBeDefined();
  });

  it('marks the active tab with aria-current and selects on tap', () => {
    const onSelect = vi.fn();
    render(<TabBar items={TAB_ITEMS} activeKey="home" onSelect={onSelect} />);
    expect(screen.getByRole('button', { name: /Home/ }).getAttribute('aria-current')).toBe('page');

    fireEvent.click(screen.getByRole('button', { name: /Library/ }));
    expect(onSelect).toHaveBeenCalledWith('library');
  });

  it('keeps a disabled tab visible, announced and inert', () => {
    const onSelect = vi.fn();
    render(<TabBar items={TAB_ITEMS} activeKey="home" onSelect={onSelect} />);
    const vault = screen.getByRole('button', { name: 'Vault' });
    expect(vault.getAttribute('aria-disabled')).toBe('true');
    fireEvent.click(vault);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('shows no create affordance unless the host provides one', () => {
    render(<TabBar items={TAB_ITEMS} activeKey="home" onSelect={() => {}} />);
    expect(screen.queryByRole('button', { name: 'New snippet' })).toBeNull();
  });

  it('renders the centre create square when onCreate is passed', () => {
    const onCreate = vi.fn();
    render(<TabBar items={TAB_ITEMS} activeKey="home" onSelect={() => {}} onCreate={onCreate} />);

    const create = screen.getByRole('button', { name: 'New snippet' });
    // The cross is pure CSS on an ink square — no icon and no caret markup.
    expect(create.querySelectorAll('svg, img')).toHaveLength(0);
    expect(create.querySelectorAll('.tv-caret')).toHaveLength(0);
    // Its slot sits between the 2nd and 3rd tab.
    const slot = create.parentElement;
    expect(slot?.previousElementSibling?.textContent).toContain('Library');
    expect(slot?.nextElementSibling?.textContent).toContain('Vault');

    fireEvent.click(create);
    expect(onCreate).toHaveBeenCalledTimes(1);
  });

  it('keeps the create button keyboard-focusable', () => {
    render(<TabBar items={TAB_ITEMS} activeKey="home" onSelect={() => {}} onCreate={() => {}} />);
    const create = screen.getByRole('button', { name: 'New snippet' });
    create.focus();
    expect(document.activeElement).toBe(create);
    expect(create.tabIndex).toBe(0);
  });

  it('speaks its own labels in Chinese under the zh locale', () => {
    render(
      <I18nProvider locale="zh">
        <TabBar items={TAB_ITEMS} activeKey="home" onSelect={() => {}} onCreate={() => {}} />
      </I18nProvider>,
    );
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeDefined();
    expect(screen.getByRole('button', { name: '新建片段' })).toBeDefined();
  });
});

describe('SnippetStrip', () => {
  it('renders title, mono preview and the caps meta line; the preview is reader-hidden', () => {
    const { container } = render(
      <SnippetStrip
        title="Nginx log tail"
        preview="docker logs -f --tail 200 nginx"
        previewMono
        meta="Command · 2 min"
        trailing="↵"
      />,
    );
    const preview = container.querySelector('.tv-strip-preview');
    expect(preview?.getAttribute('aria-hidden')).toBe('true');
    expect(preview?.classList.contains('is-mono')).toBe(true);
    expect(container.querySelector('.tv-strip-meta')?.textContent).toBe('Command · 2 min');
    expect(container.querySelector('.tv-strip-trailing')?.textContent).toBe('↵');
  });

  it('renders a locked strip as dots with no preview text and stays inert', () => {
    const onPress = vi.fn();
    const { container } = render(
      <SnippetStrip title="Prod read replica" previewDots disabled onPress={onPress} />,
    );
    expect(container.querySelector('.tv-strip-dots')).not.toBeNull();
    expect(container.querySelector('.tv-strip-preview')).toBeNull();
    fireEvent.click(screen.getByRole('button'));
    expect(onPress).not.toHaveBeenCalled();
  });

  it('fires onPress for an enabled strip and its skeleton has no spinner', () => {
    const onPress = vi.fn();
    const { container } = render(
      <>
        <SnippetStrip title="Docker tail logs" onPress={onPress} />
        <SnippetStripSkeleton />
      </>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(onPress).toHaveBeenCalledTimes(1);
    expect(container.querySelectorAll('.tv-strip-skeleton')).toHaveLength(1);
    expect(container.querySelectorAll('svg, img')).toHaveLength(0);
  });
});

describe('SearchField', () => {
  it('is a real input without any decorative caret', () => {
    const onChange = vi.fn();
    const { container, rerender } = render(
      <SearchField value="" onChange={onChange} placeholder="Search everything" label="Search" />,
    );
    // The native input caret is the only cursor.
    expect(container.querySelector('.tv-caret')).toBeNull();

    fireEvent.change(screen.getByRole('textbox', { name: 'Search' }), {
      target: { value: 'docker' },
    });
    expect(onChange).toHaveBeenCalledWith('docker');

    rerender(
      <SearchField
        value="docker"
        onChange={onChange}
        placeholder="Search everything"
        label="Search"
      />,
    );
    expect(container.querySelector('.tv-caret')).toBeNull();
  });
});

describe('FilterChipRow', () => {
  const CHIPS = [
    { key: 'all', label: 'All' },
    { key: 'command', label: 'Command' },
  ];

  it('presses exactly the active chip and selects on tap', () => {
    const onSelect = vi.fn();
    render(<FilterChipRow chips={CHIPS} activeKey="all" onSelect={onSelect} label="Filter" />);
    expect(screen.getByRole('button', { name: 'All' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('button', { name: 'Command' }).getAttribute('aria-pressed')).toBe(
      'false',
    );

    fireEvent.click(screen.getByRole('button', { name: 'Command' }));
    expect(onSelect).toHaveBeenCalledWith('command');
  });

  it('goes fully inert when the row is disabled', () => {
    const onSelect = vi.fn();
    render(
      <FilterChipRow chips={CHIPS} activeKey="all" onSelect={onSelect} label="Filter" disabled />,
    );
    const chip = screen.getByRole('button', { name: 'Command' });
    expect(chip.getAttribute('aria-disabled')).toBe('true');
    fireEvent.click(chip);
    expect(onSelect).not.toHaveBeenCalled();
  });
});

describe('markForType', () => {
  it('maps core type strings to marks and unknown values to text', () => {
    expect(markForType('command')).toBe('CM');
    expect(markForType('sensitive')).toBe('SC');
    expect(markForType('something_new')).toBe('TX');
  });
});

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Popup: search-first list over the native host, fully keyboard operable.
 * Every keystroke replaces results instantly (no debounce, panel parity);
 * Enter inserts (template variables collected first), and a failed DOM
 * insertion falls back to copy.
 */
import { makeTr } from './i18n.js';
import type {
  HostRequest,
  HostResponse,
  HostResultEntry,
  InsertReply,
  WorkerMessage,
} from './messages.js';

const tr = makeTr(navigator.language);

function callWorker<T>(message: WorkerMessage): Promise<T> {
  return chrome.runtime.sendMessage(message) as Promise<T>;
}

function callHost(request: HostRequest): Promise<HostResponse> {
  return callWorker<HostResponse>({ kind: 'host', request });
}

function byId(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (element === null) throw new Error(`missing #${id}`);
  return element;
}

const searchInput = byId('search') as HTMLInputElement;
const listElement = byId('results');
const noteElement = byId('note');
const formElement = byId('variables');

let results: HostResultEntry[] = [];
let selected = 0;

function setNote(text: string, tone: 'meta' | 'error' = 'meta'): void {
  noteElement.textContent = text;
  noteElement.dataset['tone'] = tone;
}

function renderList(): void {
  listElement.replaceChildren();
  results.forEach((entry, index) => {
    const row = document.createElement('li');
    row.className = 'row';
    if (index === selected) row.dataset['selected'] = 'true';
    const mark = document.createElement('span');
    mark.className = 'mark';
    mark.textContent = markOf(entry.snippet_type);
    const title = document.createElement('span');
    title.className = 'title';
    title.textContent = entry.title;
    const preview = document.createElement('span');
    preview.className = 'preview';
    preview.setAttribute('aria-hidden', 'true');
    preview.textContent = entry.preview;
    row.append(mark, title, preview);
    if (entry.trigger !== null) {
      const trigger = document.createElement('span');
      trigger.className = 'trigger';
      trigger.textContent = entry.trigger;
      row.append(trigger);
    }
    row.addEventListener('click', () => {
      selected = index;
      void choose();
    });
    listElement.append(row);
  });
  if (results.length === 0) {
    setNote(tr('No matches. The caret is patient.', '没有匹配。光标在等你。'));
  } else {
    setNote(
      tr(`${results.length} result${results.length === 1 ? '' : 's'}`, `${results.length} 条结果`),
    );
  }
}

/** Two-letter mono marks from the design system (frontend rule 7). */
function markOf(snippetType: string): string {
  const marks: Record<string, string> = {
    text: 'TX',
    code: 'CD',
    command: 'CM',
    prompt: 'PR',
    template: 'TP',
    ai_action: 'AI',
    link: 'LK',
  };
  return marks[snippetType] ?? 'TX';
}

async function refresh(): Promise<void> {
  const query = searchInput.value;
  const response = await callHost({ type: 'search', query, limit: 50 });
  if (!response.ok) {
    results = [];
    renderList();
    setNote(errorCopy(response.code), 'error');
    return;
  }
  if ('results' in response) {
    results = response.results;
    selected = 0;
    renderList();
  }
}

function errorCopy(code: string): string {
  if (code === 'host_unreachable') {
    return tr(
      'Your snippets are safe in the app. Turn on browser integration in Typvia settings.',
      '片段安好地待在应用里。请在 Typvia 设置中开启浏览器集成。',
    );
  }
  if (code === 'snapshot_unavailable') {
    return tr(
      'Nothing shared with the browser yet. Turn on browser integration in Typvia settings.',
      '尚未向浏览器提供数据。请在 Typvia 设置中开启浏览器集成。',
    );
  }
  return tr(
    'The extension hit a snag; the app itself is fine.',
    '扩展遇到问题；应用本身一切正常。',
  );
}

/** Collects template variable values via an inline form, then resolves. */
function collectVariables(names: string[]): Promise<Record<string, string> | null> {
  return new Promise((resolve) => {
    formElement.replaceChildren();
    formElement.hidden = false;
    listElement.hidden = true;
    const inputs = new Map<string, HTMLInputElement>();
    for (const name of names) {
      const label = document.createElement('label');
      label.textContent = name;
      const input = document.createElement('input');
      input.type = 'text';
      input.name = name;
      label.append(input);
      formElement.append(label);
      inputs.set(name, input);
    }
    const done = document.createElement('button');
    done.type = 'submit';
    done.textContent = tr('Insert', '插入');
    formElement.append(done);
    const finish = (values: Record<string, string> | null) => {
      formElement.hidden = true;
      formElement.replaceChildren();
      listElement.hidden = false;
      searchInput.focus();
      resolve(values);
    };
    formElement.onsubmit = (event) => {
      event.preventDefault();
      const values: Record<string, string> = {};
      for (const [name, input] of inputs) values[name] = input.value;
      finish(values);
    };
    formElement.onkeydown = (event) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        finish(null);
      }
    };
    inputs.values().next().value?.focus();
  });
}

async function choose(): Promise<void> {
  const entry = results[selected];
  if (entry === undefined) return;
  let variables: Record<string, string> = {};
  if (entry.variables.length > 0) {
    const collected = await collectVariables(entry.variables);
    if (collected === null) return;
    variables = collected;
  }
  const rendered = await callHost({ type: 'render', snippet_id: entry.id, variables });
  if (!rendered.ok) {
    setNote(
      rendered.code === 'missing_variable'
        ? tr('A field was left empty.', '有字段留空了。')
        : errorCopy(rendered.code),
      'error',
    );
    return;
  }
  if (!('text' in rendered)) return;
  const reply = await callWorker<InsertReply>({ kind: 'insert', text: rendered.text });
  if (reply.inserted) {
    window.close();
    return;
  }
  // Copy fallback: the page refused DOM insertion; the snippet is
  // still delivered, just one paste away.
  try {
    await navigator.clipboard.writeText(rendered.text);
    setNote(
      tr(
        'Copied — this page blocks direct insert. Paste to finish.',
        '已复制——该页面不接受直接插入，粘贴即可完成。',
      ),
    );
  } catch {
    setNote(
      tr(
        'This page blocks direct insert and copy; open the app panel instead.',
        '该页面不接受插入与复制；请使用应用面板。',
      ),
      'error',
    );
  }
}

searchInput.addEventListener('input', () => void refresh());
searchInput.addEventListener('keydown', (event) => {
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    selected = Math.min(selected + 1, results.length - 1);
    renderList();
  } else if (event.key === 'ArrowUp') {
    event.preventDefault();
    selected = Math.max(selected - 1, 0);
    renderList();
  } else if (event.key === 'Enter') {
    event.preventDefault();
    void choose();
  }
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') window.close();
});

searchInput.setAttribute('aria-label', tr('Search snippets', '搜索片段'));
searchInput.focus();
void refresh();

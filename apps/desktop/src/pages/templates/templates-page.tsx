// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { listSnippetPage, type Snippet } from '@typvia/shared';
import { Caret, useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import './templates.css';

type LoadState = 'loading' | 'ready' | 'error';

/**
 * Templates index — the list of template snippets, each linking to its
 * Builder. The design covers the Builder, not a distinct index layout, so
 * this follows the design system's list language (52px rows, caret empty
 * state, no icons).
 */
export function TemplatesPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const [load, setLoad] = useState<LoadState>('loading');
  const [templates, setTemplates] = useState<Snippet[]>([]);

  useEffect(() => {
    let live = true;
    void listSnippetPage('all', null, 'template', 200, 0)
      .then((rows) => {
        if (!live) return;
        setTemplates(rows);
        setLoad('ready');
      })
      .catch(() => {
        if (live) setLoad('error');
      });
    return () => {
      live = false;
    };
  }, []);

  return (
    <main className="tv-templates">
      <header className="tv-templates-head">
        <h1 className="tv-templates-title-en">{tr('Template builder', '模板编辑器')}</h1>
      </header>

      {load === 'loading' && (
        <p className="tv-templates-status">{tr('Loading templates…', '正在加载模板…')}</p>
      )}
      {load === 'error' && (
        <p className="tv-templates-status tv-templates-status--error">
          {tr(
            'Your templates are safe — the list just could not load. Try again.',
            '你的模板都还在——只是列表暂时无法加载。请重试。',
          )}
        </p>
      )}
      {load === 'ready' && templates.length === 0 && (
        <p className="tv-templates-empty">
          <Caret height={15} />
          {tr('No templates yet. Give a snippet the ', '还没有模板。将片段设为')}
          <strong>{tr('Template', '模板')}</strong>
          {tr(' type, then mark what changes.', '类型,然后标记会变化的部分。')}
        </p>
      )}
      {load === 'ready' && templates.length > 0 && (
        <ul className="tv-templates-list">
          {templates.map((t) => (
            <li key={t.id}>
              <button
                type="button"
                className="tv-templates-row"
                onClick={() => void navigate(`/templates/${t.id}`)}
              >
                <span className="tv-templates-mark" aria-hidden="true">
                  TP
                </span>
                <span className="tv-templates-row-title">{t.title}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

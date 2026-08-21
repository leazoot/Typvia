// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  getSnippet,
  templateFields,
  templatePreview,
  templateSaveFields,
  templateVariables,
  updateSnippet,
  type Snippet,
  type TemplateField,
  type VariableProposal,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router';
import { Extract } from './extract';
import { FieldCard } from './field-card';
import './templates.css';

type LoadState = 'loading' | 'ready' | 'error';

/** Merges detected body variables (in order) with existing field config,
 *  preserving edits by name and dropping fields whose variable is gone — the
 *  field set always mirrors the body's `{{var}}` tokens. */
function mergeFields(variables: string[], current: TemplateField[]): TemplateField[] {
  const byName = new Map(current.map((f) => [f.name, f]));
  return variables.map((name, i) => {
    const existing = byName.get(name);
    if (existing) return { ...existing, sortOrder: i };
    return {
      id: '',
      name,
      label: name,
      fieldType: 'single_line_text',
      defaultValue: null,
      options: [],
      validation: null,
      isRequired: false,
      sortOrder: i,
      platformOverrides: null,
    };
  });
}

/** Splits body text for display, tagging each `{{var}}` span with its 1-based
 *  number from the authoritative variable order. Cosmetic only. */
function bodyNodes(body: string, order: string[]): { text: string; num: number | null }[] {
  const parts: { text: string; num: number | null }[] = [];
  const re = /\{\{\s*([^{}]+?)\s*\}\}/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(body)) !== null) {
    if (m.index > last) parts.push({ text: body.slice(last, m.index), num: null });
    const idx = order.indexOf(m[1] ?? '');
    parts.push({ text: m[0], num: idx >= 0 ? idx + 1 : null });
    last = m.index + m[0].length;
  }
  if (last < body.length) parts.push({ text: body.slice(last), num: null });
  return parts;
}

/**
 * Template Builder — body on the left (variables marked
 * inline), field configuration in the centre, live preview on the right. The
 * preview always resolves with defaults; secret references show a mask.
 *
 * Design deviations: body marking is done by typing `{{name}}` in
 * the text (an "Edit text" toggle swaps the token view for a textarea) rather
 * than a select-to-mark gesture; conditional fields ("Only when") and the
 * accent-coloured resolved values in the preview are deferred; the right-rail
 * "Insert into…" call belongs to the fill-and-inject flow.
 */
export function TemplateBuilder() {
  const tr = useTr();
  const { id = '' } = useParams();
  const [load, setLoad] = useState<LoadState>('loading');
  const [snippet, setSnippet] = useState<Snippet | null>(null);
  const [body, setBody] = useState('');
  const [fields, setFields] = useState<TemplateField[]>([]);
  const [values, setValues] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState('');
  const [editing, setEditing] = useState(false);
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const fieldsRef = useRef<TemplateField[]>([]);
  fieldsRef.current = fields;

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const [snap, saved] = await Promise.all([getSnippet(id), templateFields(id)]);
        if (!live) return;
        const text = snap.body ?? '';
        const vars = await templateVariables(text);
        if (!live) return;
        setSnippet(snap);
        setBody(text);
        setFields(mergeFields(vars, saved));
        setEditing(text.trim() === '');
        setLoad('ready');
      } catch {
        if (live) setLoad('error');
      }
    })();
    return () => {
      live = false;
    };
  }, [id]);

  // Re-detect variables after the body settles, re-merging with current config.
  useEffect(() => {
    if (load !== 'ready') return;
    const t = setTimeout(() => {
      void templateVariables(body)
        .then((vars) => setFields(mergeFields(vars, fieldsRef.current)))
        .catch(() => undefined);
    }, 250);
    return () => clearTimeout(t);
  }, [body, load]);

  // Live preview whenever the body, field config, or fill values change.
  useEffect(() => {
    if (load !== 'ready') return;
    const t = setTimeout(() => {
      void templatePreview(body, fields, values)
        .then(setPreview)
        .catch(() => undefined);
    }, 150);
    return () => clearTimeout(t);
  }, [body, fields, values, load]);

  const updateField = useCallback((next: TemplateField) => {
    setFields((prev) => prev.map((f) => (f.name === next.name ? next : f)));
  }, []);

  // Applies one AI proposal locally: the span becomes `{{name}}` in the
  // body and the merged field takes the proposed type/default. Persisting
  // still requires the builder's own Save (nothing stored before that).
  const applyProposal = useCallback(
    (proposal: VariableProposal) => {
      const newBody = body.split(proposal.original).join(`{{${proposal.name}}}`);
      setBody(newBody);
      void templateVariables(newBody)
        .then((vars) => {
          setFields((prev) =>
            mergeFields(vars, prev).map((f) =>
              f.name === proposal.name
                ? { ...f, fieldType: proposal.fieldType, defaultValue: proposal.defaultValue }
                : f,
            ),
          );
        })
        .catch(() => {
          // The debounced re-detect effect converges the field list.
        });
    },
    [body],
  );

  const save = useCallback(async () => {
    if (!snippet) return;
    setSaveState('saving');
    try {
      await updateSnippet({
        id: snippet.id,
        title: snippet.title,
        body,
        snippetType: snippet.snippetType,
        description: snippet.description,
        folderId: snippet.folderId,
        trigger: snippet.trigger,
        triggerMode: snippet.triggerMode,
        language: snippet.language,
        isFavorite: snippet.isFavorite,
        isPinned: snippet.isPinned,
        isEnabled: snippet.isEnabled,
      });
      const stored = await templateSaveFields(snippet.id, fields);
      setFields((prev) =>
        mergeFields(
          prev.map((f) => f.name),
          stored,
        ),
      );
      setSaveState('saved');
      setTimeout(() => setSaveState((s) => (s === 'saved' ? 'idle' : s)), 1400);
    } catch {
      setSaveState('error');
    }
  }, [snippet, body, fields]);

  if (load === 'loading') {
    return (
      <main className="tv-builder tv-builder--status">
        <p className="tv-builder-skeleton">{tr('Loading template…', '正在加载模板…')}</p>
      </main>
    );
  }
  if (load === 'error' || !snippet) {
    return (
      <main className="tv-builder tv-builder--status">
        <p className="tv-builder-error">
          {tr(
            'This template is still here — it just could not be loaded. Try again.',
            '这个模板仍然保存着——只是暂时无法加载。请重试。',
          )}
        </p>
      </main>
    );
  }

  const order = fields.map((f) => f.name);

  return (
    <main className="tv-builder">
      <header className="tv-builder-head">
        <div className="tv-builder-crumb">
          <span className="tv-builder-crumb-root">{tr('Templates', '模板')}</span>
          <span className="tv-builder-crumb-sep">·</span>
          <span className="tv-builder-crumb-name">{snippet.title}</span>
        </div>
        <button type="button" className="tv-builder-save" onClick={() => void save()}>
          {saveState === 'saving'
            ? tr('Saving…', '保存中…')
            : saveState === 'saved'
              ? tr('Saved', '已保存')
              : saveState === 'error'
                ? tr('Not saved', '未保存')
                : tr('Save', '保存')}
        </button>
      </header>

      <div className="tv-builder-grid">
        <section className="tv-builder-col" aria-label={tr('Template text', '模板文本')}>
          <div className="tv-builder-step">
            <span className="tv-builder-step-en">{tr('Write the text', '编写文本')}</span>
            <button
              type="button"
              className="tv-builder-edit-toggle"
              onClick={() => setEditing((e) => !e)}
            >
              {editing ? tr('Done', '完成') : tr('Edit text', '编辑文本')}
            </button>
          </div>
          {editing ? (
            <textarea
              className="tv-builder-body-input"
              aria-label={tr('Template body', '模板正文')}
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder={tr(
                'Write the text, then mark what changes with {{name}}',
                '先编写文本,再用 {{name}} 标记会变化的部分',
              )}
            />
          ) : (
            <div className="tv-builder-body-view">
              {body.trim() === '' ? (
                <span className="tv-builder-body-empty">
                  {tr(
                    'Nothing yet. Choose “Edit text” and write the prose.',
                    '还没有内容。选择「编辑文本」开始编写。',
                  )}
                </span>
              ) : (
                bodyNodes(body, order).map((part, i) =>
                  part.num === null ? (
                    <span key={i}>{part.text}</span>
                  ) : (
                    <span key={i} className="tv-builder-token">
                      {part.text}
                      <span className="tv-builder-token-num" aria-hidden="true">
                        {part.num}
                      </span>
                    </span>
                  ),
                )
              )}
            </div>
          )}
        </section>

        <section className="tv-builder-col" aria-label={tr('Fields', '字段')}>
          <div className="tv-builder-step">
            <span className="tv-builder-step-en">
              {tr('Mark what changes', '标记会变化的部分')}
            </span>
          </div>
          {fields.length === 0 ? (
            <p className="tv-builder-nofields">
              {tr('No variables yet. Wrap the parts that change in ', '还没有变量。用 ')}
              <code>{'{{ }}'}</code>
              {tr(
                ' — a field appears here for each one.',
                ' 包裹会变化的部分——每个变量都会在这里生成一个字段。',
              )}
            </p>
          ) : (
            <div className="tv-builder-cards">
              {fields.map((field, i) => (
                <FieldCard key={field.name} field={field} index={i + 1} onChange={updateField} />
              ))}
            </div>
          )}
          <Extract body={body} onApply={applyProposal} />
        </section>

        <section className="tv-builder-col" aria-label={tr('Preview', '预览')}>
          <div className="tv-builder-step">
            <span className="tv-builder-step-en">{tr('Preview', '预览')}</span>
          </div>
          {fields.length > 0 && (
            <div className="tv-builder-fills">
              {fields.map((field) => (
                <label key={field.name} className="tv-builder-fill">
                  <span className="tv-builder-fill-key">{field.name}</span>
                  <input
                    className="tv-field-input"
                    value={values[field.name] ?? ''}
                    placeholder={field.defaultValue ?? field.label}
                    onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
                  />
                </label>
              ))}
            </div>
          )}
          <pre className="tv-builder-preview" aria-live="polite">
            {preview}
          </pre>
        </section>
      </div>
    </main>
  );
}

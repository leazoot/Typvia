// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { clipboardReadText, createSnippet } from '@typvia/shared';
import { TypeMark, useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';

interface FirstSnippetStepProps {
  onContinue: () => void;
}

const TITLE_MAX = 60;

/** Title fallback derived from the first content line — plain text, no AI. */
function titleFromBody(body: string): string {
  const first = body.split('\n', 1)[0]?.trim() ?? '';
  return first.length > TITLE_MAX ? `${first.slice(0, TITLE_MAX - 1)}…` : first;
}

/**
 * Step 3 — save the first snippet. The card is prefilled from the clipboard
 * when the host offers a seed (suspected secrets never come back, host-side
 * filter). Recorded deviation: the design's on-device-AI metadata rail is
 * replaced by an honest static note until AI lands.
 */
export function FirstSnippetStep({ onContinue }: FirstSnippetStepProps) {
  const tr = useTr();
  // null = clipboard read pending; '' = nothing usable, start empty.
  const [seed, setSeed] = useState<string | null>(null);
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);

  const applySeed = (text: string | null) => {
    setSeed(text ?? '');
    if (text !== null) {
      setBody(text);
      setTitle(titleFromBody(text));
    }
  };

  useEffect(() => {
    let cancelled = false;
    clipboardReadText()
      .then((text) => {
        if (!cancelled) applySeed(text);
      })
      .catch(() => {
        if (!cancelled) setSeed('');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const rereadClipboard = () => {
    clipboardReadText()
      .then(applySeed)
      .catch(() => {
        setSeed('');
      });
  };

  const canSave = body.trim().length > 0 && !saving;

  const save = () => {
    if (!canSave) return;
    setSaving(true);
    setSaveError(false);
    const finalTitle = title.trim() === '' ? titleFromBody(body) : title.trim();
    createSnippet({ title: finalTitle, body, snippetType: 'text' })
      .then(onContinue)
      .catch(() => {
        setSaveError(true);
        setSaving(false);
      });
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) save();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  });

  const prefilled = seed !== null && seed !== '';

  return (
    <>
      <section className="tv-onb-body tv-onb-split">
        <div className="tv-onb-split-main">
          <h1 className="tv-onb-title">{tr('Save your first one.', '保存第一条片段。')}</h1>
          <p className="tv-onb-lead">
            {seed === null
              ? tr('Checking your clipboard…', '正在检查剪贴板…')
              : prefilled
                ? tr(
                    'We found this on your clipboard. Edit anything, or paste something else.',
                    '我们在剪贴板上发现了这段内容。可以随意编辑，或粘贴其他内容。',
                  )
                : tr(
                    'Nothing usable on the clipboard — type something you retype often.',
                    '剪贴板上没有可用内容——输入一段你经常重复输入的文本。',
                  )}
          </p>
          <div className="tv-onb-editor">
            <div className="tv-onb-editor-head">
              <TypeMark code="TX" />
              <input
                className="tv-onb-editor-title"
                aria-label={tr('Snippet title', '片段标题')}
                placeholder={tr('Name it', '起个名字')}
                value={title}
                onChange={(event) => setTitle(event.target.value)}
              />
              {prefilled && (
                <span className="tv-onb-editor-meta">
                  {tr('title from the first line', '标题取自第一行')}
                </span>
              )}
            </div>
            <textarea
              className="tv-onb-editor-body"
              aria-label={tr('Snippet content', '片段内容')}
              placeholder={tr(
                'Paste or type the text you keep retyping',
                '粘贴或输入你反复输入的文本',
              )}
              rows={5}
              value={body}
              onChange={(event) => setBody(event.target.value)}
            />
          </div>
          {saveError && (
            <p className="tv-onb-error" role="alert">
              {tr(
                'Nothing was saved — it could not be written. Your text is still above; try again.',
                '没有保存任何内容——写入失败。你的文本仍保留在上方，可以再试一次。',
              )}
            </p>
          )}
          <div className="tv-onb-actions">
            <button type="button" className="tv-onb-primary" disabled={!canSave} onClick={save}>
              {tr('Save it', '保存')}
              <kbd>⌘↵</kbd>
            </button>
            <button type="button" className="tv-onb-ghost" onClick={rereadClipboard}>
              {tr('Paste something else', '粘贴其他内容')}
            </button>
            <button type="button" className="tv-onb-skip" onClick={onContinue}>
              {tr("Skip — I'll save one later", '跳过——稍后再保存')}
            </button>
          </div>
        </div>
        <aside className="tv-onb-rail">
          <h2 className="tv-onb-rail-label">{tr('Where this went', '内容去了哪里')}</h2>
          <div className="tv-onb-rail-rows">
            <div className="tv-onb-rail-row">
              <span>{tr('Read from your clipboard', '从你的剪贴板读取')}</span>
              <span className="tv-onb-rail-sub">
                {tr('on this Mac · nothing sent anywhere', '仅在这台 Mac 上 · 不发送到任何地方')}
              </span>
            </div>
            <div className="tv-onb-rail-row">
              <span>{tr('Saved to your library', '保存到你的库')}</span>
              <span className="tv-onb-rail-sub">
                {tr('a local database on this Mac', '这台 Mac 上的本地数据库')}
              </span>
            </div>
            <div className="tv-onb-rail-row">
              <span>{tr('Suspected secrets stay out', '疑似密钥被排除在外')}</span>
              <span className="tv-onb-rail-sub">
                {tr(
                  'a copied token never prefills this form',
                  '复制的 Token 永远不会预填到这个表单',
                )}
              </span>
            </div>
          </div>
        </aside>
      </section>
      <footer className="tv-onb-foot" />
    </>
  );
}

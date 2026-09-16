// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { clipboardReadText, createSnippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { TextAction } from '../../paper/kit';
import { StepBody } from './step-kit';

const TITLE_MAX = 60;

/** Title fallback derived from the first content line — plain text, no AI. */
function titleFromBody(body: string): string {
  const first = body.split('\n', 1)[0]?.trim() ?? '';
  return first.length > TITLE_MAX ? `${first.slice(0, TITLE_MAX - 1)}…` : first;
}

/**
 * Step 3 — the first words. The body is prefilled from the clipboard when the
 * host offers it; suspected secrets never come back from the host.
 */
export function FirstSnippetStep({
  onContinue,
  onImport,
}: {
  onContinue: () => void;
  /** Ends setup in Settings, where importing from other tools lives. */
  onImport: () => void;
}) {
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
      .catch(() => setSeed(''));
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
    return () => window.removeEventListener('keydown', onKey);
  });

  const prefilled = seed !== null && seed !== '';

  return (
    <StepBody
      mascot="idle"
      title={tr('Give it its first words.', '给它第一句话。')}
      actions={
        <>
          <TextAction primary disabled={!canSave} onClick={save}>
            {tr('Save it', '存下来')}
          </TextAction>
          <TextAction onClick={rereadClipboard}>
            {tr('Take the clipboard again', '换成剪贴板里的内容')}
          </TextAction>
          <TextAction onClick={onImport}>
            {tr('Bring them in from another tool', '从别的工具导进来')}
          </TextAction>
          <TextAction onClick={onContinue}>
            {tr('Skip · save one later', '跳过 · 以后再存')}
          </TextAction>
        </>
      }
    >
      <p>
        {seed === null
          ? tr('Looking at your clipboard…', '正在看剪贴板…')
          : prefilled
            ? tr(
                'We found this on your clipboard. Change anything, or take something else.',
                '剪贴板里正好有这一段。随便改,或者换成别的。',
              )
            : tr(
                'Nothing usable on the clipboard — type the sentence you typed twice today. Empty is fine too; it fills up as you go.',
                '剪贴板里没有能用的——把你今天重复打了第二遍的那句话写进来。空着也行,用起来自然会攒。',
              )}
      </p>
      <div className="tvo-form">
        <label className="tvo-line">
          <span className="tvo-line-label">{tr('Name', '名字')}</span>
          <input
            aria-label={tr('Snippet title', '片段标题')}
            placeholder={tr('Name it', '起个名字')}
            value={title}
            spellCheck={false}
            onChange={(event) => setTitle(event.target.value)}
          />
        </label>
        <textarea
          className="tvo-body"
          aria-label={tr('Snippet content', '片段内容')}
          placeholder={tr('The words you keep typing', '你反复打的那句话')}
          rows={5}
          value={body}
          onChange={(event) => setBody(event.target.value)}
        />
        {prefilled && (
          <p className="tvo-hint">
            {tr(
              'Read from the clipboard on this Mac; nothing was sent anywhere. A copied token never fills this in. ⌘⏎ saves.',
              '从这台 Mac 的剪贴板读的,没发到任何地方。复制过的 Token 永远不会填进来。⌘⏎ 存下来。',
            )}
          </p>
        )}
      </div>
      {saveError && (
        <p className="tvo-said" role="alert">
          {tr(
            'Nothing was saved — it could not be written. Your words are still above; try again.',
            '没存上——写不进去。你写的还在上面,再试一次。',
          )}
        </p>
      )}
    </StepBody>
  );
}

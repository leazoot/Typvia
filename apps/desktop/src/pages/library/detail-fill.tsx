// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { templateFields, type Snippet, type TemplateField } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState, type KeyboardEvent } from 'react';
import { TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';

/**
 * A template's variables filled in right in the detail pane: one line under
 * each, ⇥ to the next, ⏎ inserts into the front app. Insert waits for every
 * required value; a vault field cannot be filled from here and says so.
 */
export function DetailFill({
  snippet,
  onInsert,
  onCancel,
}: {
  snippet: Snippet;
  onInsert: (values: Record<string, string>) => void;
  onCancel: () => void;
}) {
  const tr = useTr();
  const [fields, setFields] = useState<TemplateField[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [values, setValues] = useState<Record<string, string>>({});

  useEffect(() => {
    let live = true;
    templateFields(snippet.id)
      .then((rows) => {
        if (!live) return;
        setFields(rows);
        setValues(Object.fromEntries(rows.map((field) => [field.name, field.defaultValue ?? ''])));
      })
      .catch(() => {
        if (live) setFailed(true);
      });
    return () => {
      live = false;
    };
  }, [snippet.id]);

  const hasSecret = fields?.some((field) => field.fieldType === 'secret_ref') ?? false;
  const missing =
    fields?.some((field) => field.isRequired && (values[field.name] ?? '').trim() === '') ?? true;
  const ready = fields !== null && !hasSecret && !missing;

  const submit = () => {
    if (ready) onInsert(values);
  };

  // The page's own ⏎ would insert the raw body, so these keys stop here.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onCancel();
      return;
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      event.stopPropagation();
      submit();
    }
  };

  return (
    <div className="tvl-fill" onKeyDown={onKeyDown}>
      <div className="tvl-fill-head">
        <Mascot state="typing" size={22} />
        <span className="tpi-eyebrow">{tr('Fill the variables', '填变量')}</span>
      </div>
      {failed ? (
        <p className="tvl-detail-note">
          {tr(
            'The variables could not be read. The snippet is unchanged.',
            '变量没读出来。片段没有变。',
          )}
        </p>
      ) : (
        fields?.map((field, index) => (
          <label key={field.name} className="tvl-fill-field">
            <span className="tvl-fill-label">
              {field.label}
              {field.isRequired && ` · ${tr('required', '必须填')}`}
            </span>
            {field.fieldType === 'secret_ref' ? (
              <span className="tvl-fill-line is-static">{tr('Needs the vault', '需要保险库')}</span>
            ) : (
              <input
                className="tvl-fill-line"
                value={values[field.name] ?? ''}
                placeholder={field.isRequired ? '' : tr('Can stay empty', '留空也行')}
                autoFocus={index === 0}
                spellCheck={false}
                onChange={(event) =>
                  setValues((current) => ({ ...current, [field.name]: event.target.value }))
                }
              />
            )}
          </label>
        ))
      )}
      {hasSecret && (
        <p className="tvl-detail-note">
          {tr(
            'This template uses a vault field, so it cannot be inserted from here yet.',
            '这个模板用到了保险库里的字段,暂时不能从这里插入。',
          )}
        </p>
      )}
      <div className="tvl-fill-actions">
        <TextAction primary disabled={!ready} onClick={submit}>
          {tr('Insert into the front app', '插入到最前应用')}
        </TextAction>
        <TextAction onClick={onCancel}>{tr('Not now', '先不')}</TextAction>
        <span className="tvl-grow" />
        <span className="tvl-detail-hint">{tr('⇥ next · ⏎ insert', '⇥ 下一格 · ⏎ 插入')}</span>
      </div>
    </div>
  );
}

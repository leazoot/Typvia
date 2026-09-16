// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { panelInsertTemplate, type Snippet, type TemplateField } from '@typvia/shared';
import { counted, useTr } from '@typvia/ui';
import { useMemo, useState } from 'react';
import { KeyCap } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';
import { loadInsertMethod } from '../../workspace/insert-method';

interface PanelFillProps {
  snippet: Snippet;
  fields: TemplateField[];
  destination: string | null;
  onCancel: () => void;
}

function initialValues(fields: TemplateField[]): Record<string, string> {
  return Object.fromEntries(fields.map((f) => [f.name, f.defaultValue ?? '']));
}

/**
 * Filling a template's variables over the app it will land in: one line per
 * variable, ⇥ to the next, ⏎ types it out. Insert waits until every required
 * variable has a value; secret-reference fields need the vault, so a template
 * using them cannot be typed from here and says so. esc goes back to the list.
 */
export function PanelFill({ snippet, fields, destination, onCancel }: PanelFillProps) {
  const tr = useTr();
  const [values, setValues] = useState<Record<string, string>>(() => initialValues(fields));

  const hasSecret = useMemo(() => fields.some((f) => f.fieldType === 'secret_ref'), [fields]);
  const missingRequired = fields.some(
    (f) => f.isRequired && (values[f.name] ?? '').trim() === '' && (f.defaultValue ?? '') === '',
  );
  const canInsert = !hasSecret && !missingRequired;

  const submit = () => {
    if (!canInsert) return;
    // The host hides the panel and restores focus before injecting, so by the
    // time this can reject the surface that would report it is gone.
    void panelInsertTemplate(snippet.id, values, loadInsertMethod()).catch(() => undefined);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  return (
    <div className="tvq-fill" onKeyDown={onKeyDown}>
      <div className="tvq-head">
        <Mascot state="typing" size={22} />
        {snippet.trigger !== null && <span className="tvq-mono">{snippet.trigger}</span>}
        <span className="tvq-head-title">{snippet.title}</span>
        <span className="tvq-spacer" />
        <span className="tvq-flag">
          {counted(tr, fields.length, 'variable', 'variables', `${String(fields.length)} 个变量`)}
        </span>
      </div>

      <div className="tvq-fields">
        {fields.map((field, index) => (
          <label key={field.name} className="tvq-field">
            <span className="tvq-field-label">
              {field.label}
              {field.isRequired && ` · ${tr('required', '必须填')}`}
            </span>
            {field.fieldType === 'secret_ref' ? (
              <span className="tvq-field-line is-static">
                {tr('Needs the vault', '需要保险库')}
              </span>
            ) : (
              <input
                className="tvq-field-line"
                value={values[field.name] ?? ''}
                placeholder={
                  field.defaultValue ?? (field.isRequired ? '' : tr('Can stay empty', '留空也行'))
                }
                required={field.isRequired}
                autoFocus={index === 0}
                spellCheck={false}
                onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
              />
            )}
          </label>
        ))}
      </div>

      {hasSecret && (
        <p className="tvq-notice">
          {tr(
            'This template uses a vault field, so it cannot be typed out from here yet.',
            '这个模板用到了保险库里的字段,暂时不能从这里打出去。',
          )}
        </p>
      )}

      <footer className="tvq-foot is-bare">
        <KeyCap>⇥</KeyCap>
        <span className="tvq-foot-word">{tr('next', '下一格')}</span>
        <KeyCap>⏎</KeyCap>
        <span className="tvq-foot-word">
          {destination !== null
            ? tr(`type into ${destination}`, `打到 ${destination} 里`)
            : tr('type it out', '打出去')}
        </span>
        <span className="tvq-spacer" />
        <span className="tvq-foot-note">{tr('esc to go back', 'esc 收起')}</span>
      </footer>
    </div>
  );
}

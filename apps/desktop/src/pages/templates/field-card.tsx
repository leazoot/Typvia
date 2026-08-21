// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { TemplateField, TemplateFieldType } from '@typvia/shared';
import { useTr } from '@typvia/ui';

/** Field types offered in the Builder, with their (en, zh) labels. */
const FIELD_TYPES: { value: TemplateFieldType; en: string; zh: string }[] = [
  { value: 'single_line_text', en: 'Single line', zh: '单行文本' },
  { value: 'multi_line_text', en: 'Multiline', zh: '多行文本' },
  { value: 'number', en: 'Number', zh: '数字' },
  { value: 'date', en: 'Date', zh: '日期' },
  { value: 'time', en: 'Time', zh: '时间' },
  { value: 'single_select', en: 'Select', zh: '单选' },
  { value: 'multi_select', en: 'Multi-select', zh: '多选' },
  { value: 'toggle', en: 'Toggle', zh: '开关' },
  { value: 'dropdown', en: 'Dropdown', zh: '下拉选择' },
  { value: 'dynamic_variable', en: 'Dynamic', zh: '动态变量' },
  { value: 'secret_ref', en: 'Secret ref', zh: 'Secret 引用' },
];

const CHOICE_TYPES: TemplateFieldType[] = ['single_select', 'multi_select', 'dropdown'];

export function isChoiceType(type: TemplateFieldType): boolean {
  return CHOICE_TYPES.includes(type);
}

interface FieldCardProps {
  field: TemplateField;
  index: number;
  onChange: (next: TemplateField) => void;
}

/**
 * One field-configuration card in the centre column: the numbered variable,
 * its type, default, and whether it is required. Options are shown for choice
 * types. Conditional fields ("Only when") are deferred.
 */
export function FieldCard({ field, index, onChange }: FieldCardProps) {
  const tr = useTr();
  const choice = isChoiceType(field.fieldType);
  return (
    <div className="tv-field-card">
      <div className="tv-field-card-head">
        <span className="tv-field-num" aria-hidden="true">
          {index}
        </span>
        <span className="tv-field-name">{field.name}</span>
        <button
          type="button"
          className="tv-field-required"
          aria-pressed={field.isRequired}
          onClick={() => onChange({ ...field, isRequired: !field.isRequired })}
        >
          {field.isRequired ? tr('required', '必填') : tr('optional', '可选')}
        </button>
      </div>

      <label className="tv-field-row">
        <span className="tv-field-key">{tr('Field type', '字段类型')}</span>
        <select
          className="tv-field-select"
          value={field.fieldType}
          onChange={(e) => {
            const fieldType = e.target.value as TemplateFieldType;
            onChange({
              ...field,
              fieldType,
              options: isChoiceType(fieldType) ? field.options : [],
            });
          }}
        >
          {FIELD_TYPES.map((t) => (
            <option key={t.value} value={t.value}>
              {tr(t.en, t.zh)}
            </option>
          ))}
        </select>
      </label>

      <label className="tv-field-row">
        <span className="tv-field-key">{tr('Label', '显示名称')}</span>
        <input
          className="tv-field-input"
          value={field.label}
          onChange={(e) => onChange({ ...field, label: e.target.value })}
        />
      </label>

      {field.fieldType !== 'secret_ref' && (
        <label className="tv-field-row">
          <span className="tv-field-key">{tr('Default', '默认值')}</span>
          <input
            className="tv-field-input"
            value={field.defaultValue ?? ''}
            placeholder="—"
            onChange={(e) =>
              onChange({ ...field, defaultValue: e.target.value === '' ? null : e.target.value })
            }
          />
        </label>
      )}

      {choice && (
        <label className="tv-field-row">
          <span className="tv-field-key">{tr('Options', '选项')}</span>
          <input
            className="tv-field-input"
            value={field.options.join(', ')}
            placeholder={tr('low, medium, high', '低, 中, 高')}
            onChange={(e) =>
              onChange({
                ...field,
                options: e.target.value
                  .split(',')
                  .map((o) => o.trim())
                  .filter((o) => o !== ''),
              })
            }
          />
        </label>
      )}
    </div>
  );
}

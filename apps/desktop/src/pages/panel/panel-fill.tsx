import { panelInsertTemplate, type Snippet, type TemplateField } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useMemo, useRef, useState } from 'react';

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
 * Panel template fill mode. Select a template → fill its fields → ↵ renders
 * and injects into the app the panel came from. Insert is blocked until every
 * required field is filled; secret-reference fields need the vault, so a
 * template using them cannot be injected yet and says so. ESC returns to the
 * results list.
 */
export function PanelFill({ snippet, fields, destination, onCancel }: PanelFillProps) {
  const tr = useTr();
  const [values, setValues] = useState<Record<string, string>>(() => initialValues(fields));
  const firstInput = useRef<HTMLInputElement>(null);

  const hasSecret = useMemo(() => fields.some((f) => f.fieldType === 'secret_ref'), [fields]);
  const missingRequired = fields.some(
    (f) => f.isRequired && (values[f.name] ?? '').trim() === '' && (f.defaultValue ?? '') === '',
  );
  const canInsert = !hasSecret && !missingRequired;

  const submit = () => {
    if (!canInsert) return;
    // The host hides the panel and restores focus before injecting; a rejection
    // (e.g. permission) leaves the action reported, not silently lost.
    void panelInsertTemplate(snippet.id, values).catch(() => undefined);
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
    <div className="tv-panel-fill" onKeyDown={onKeyDown}>
      <div className="tv-panel-fill-head">
        <span className="tv-panel-fill-title">{snippet.title}</span>
        {destination !== null && <span className="tv-panel-dest">→ {destination}</span>}
      </div>

      <div className="tv-panel-fill-fields">
        {fields.map((field, index) => (
          <label key={field.name} className="tv-panel-fill-field">
            <span className="tv-panel-fill-key">
              {field.label}
              {field.isRequired && (
                <span className="tv-panel-fill-req" aria-hidden="true">
                  {' '}
                  ·
                </span>
              )}
            </span>
            {field.fieldType === 'secret_ref' ? (
              <span className="tv-panel-fill-secret">{tr('Needs the vault', '需要保险库')}</span>
            ) : (
              <input
                ref={index === 0 ? firstInput : undefined}
                className="tv-panel-fill-input"
                value={values[field.name] ?? ''}
                placeholder={field.defaultValue ?? ''}
                required={field.isRequired}
                autoFocus={index === 0}
                onChange={(e) => setValues((v) => ({ ...v, [field.name]: e.target.value }))}
              />
            )}
          </label>
        ))}
      </div>

      {hasSecret && (
        <p className="tv-panel-fill-note">
          {tr(
            'Secret fields are filled after unlocking the vault — coming soon.',
            '密文字段将在解锁保险库后填充——即将推出。',
          )}
        </p>
      )}

      <footer className="tv-panel-footer">
        <span className="tv-panel-keys">
          <kbd>tab</kbd>
          <span className="tv-panel-key-word">{tr('next', '下一项')}</span>
          <kbd>↵</kbd>
          <span className="tv-panel-key-word">{tr('insert', '插入')}</span>
          <kbd>esc</kbd>
          <span className="tv-panel-key-word">{tr('back', '返回')}</span>
        </span>
      </footer>
    </div>
  );
}

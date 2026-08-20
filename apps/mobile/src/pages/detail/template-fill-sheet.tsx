import { templateCopy, templateFields, templatePreview, templateVariables } from '@typvia/shared';
import type { Snippet, TemplateField } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { pushBackHandler } from '../../platform';
import './template-fill.css';

interface TemplateFillSheetProps {
  snippet: Snippet;
  onClose: () => void;
  /** After a successful copy — the host closes the sheet and toasts. */
  onCopied: () => void;
}

/** A bare `{{var}}` without a saved field definition still gets an input. */
function pseudoField(name: string, sortOrder: number): TemplateField {
  return {
    id: '',
    name,
    label: name,
    fieldType: 'single_line_text',
    defaultValue: null,
    options: [],
    validation: null,
    isRequired: false,
    sortOrder,
    platformOverrides: null,
  };
}

/**
 * Template fill sheet: the variable
 * fields under caps labels, a live preview with the filled values carried
 * on tinted accent underlines, and one primary action. In-app the primary
 * is Copy — real insertion belongs to the keyboard surface, where the
 * drawn "Insert ↵" maps instead.
 */
export function TemplateFillSheet({ snippet, onClose, onCopied }: TemplateFillSheetProps) {
  const tr = useTr();
  const [fields, setFields] = useState<TemplateField[] | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const generation = useRef(0);

  useEffect(() => pushBackHandler(onClose), [onClose]);

  // Saved field definitions first; bare {{vars}} fall back to plain inputs.
  useEffect(() => {
    let cancelled = false;
    templateFields(snippet.id)
      .then((saved) => {
        if (cancelled) return null;
        if (saved.length > 0) return saved;
        return templateVariables(snippet.body ?? '').then((names) =>
          names.map((name, index) => pseudoField(name, index)),
        );
      })
      .then((resolved) => {
        if (cancelled || resolved === null) return;
        setFields(resolved);
        const seeded: Record<string, string> = {};
        for (const field of resolved) {
          seeded[field.name] = field.defaultValue ?? '';
        }
        setValues(seeded);
      })
      .catch(() => {
        if (!cancelled) {
          setFailed(
            tr(
              'The fields could not be loaded — the template itself is unchanged.',
              '字段未能加载——模板本身没有变化。',
            ),
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, [snippet.id, snippet.body, tr]);

  // Live preview on every value change; stale answers are dropped.
  useEffect(() => {
    if (fields === null) return;
    generation.current += 1;
    const stamp = generation.current;
    setUpdating(true);
    templatePreview(snippet.body ?? '', fields, values)
      .then((rendered) => {
        if (generation.current !== stamp) return;
        setPreview(rendered);
        setUpdating(false);
      })
      .catch(() => {
        if (generation.current !== stamp) return;
        setUpdating(false);
      });
  }, [fields, values, snippet.body]);

  const copy = () => {
    templateCopy(snippet.id, values)
      .then(onCopied)
      .catch(() => {
        setFailed(tr('Copy failed — your values are still here.', '复制失败——你填写的内容还在。'));
      });
  };

  const filled = new Set(
    Object.entries(values)
      .filter(([, value]) => value.trim() !== '')
      .map(([name]) => name),
  );

  /** Preview with filled values marked (presentation only). */
  const previewNodes = () => {
    if (preview === null) return null;
    // Mark each filled value's occurrences so the eye can find them.
    const marks = Array.from(filled)
      .map((name) => values[name] ?? '')
      .filter((value) => value !== '');
    if (marks.length === 0) return preview;
    const pattern = marks.map((value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
    const parts = preview.split(new RegExp(`(${pattern})`, 'g'));
    return parts.map((part, index) =>
      marks.includes(part) ? (
        <span key={`${String(index)}-${part}`} className="tv-mtpl-filled">
          {part}
        </span>
      ) : (
        part
      ),
    );
  };

  return (
    <div
      className="tv-msheet-scrim"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className="tv-msheet tv-mtpl"
        role="dialog"
        aria-label={tr('Fill template', '填充模板')}
      >
        <div className="tv-msheet-handle" aria-hidden="true" />

        <header className="tv-mtpl-head">
          <div>
            <div className="tv-med-caps">{tr('Template', '模板')}</div>
            <h1 className="tv-mtpl-title">{snippet.title}</h1>
          </div>
          <button type="button" className="tv-med-cancel" onClick={onClose}>
            {tr('Cancel', '取消')}
          </button>
        </header>

        {failed !== null && (
          <p className="tv-mtpl-error" role="alert">
            {failed}
          </p>
        )}

        {fields !== null && (
          <div className="tv-mtpl-fields">
            {fields.map((field) => (
              <label key={field.name} className="tv-mtpl-field">
                <span className="tv-mtpl-field-label">{field.label}</span>
                {field.options.length > 0 ? (
                  <select
                    className="tv-mtpl-input"
                    value={values[field.name] ?? ''}
                    onChange={(event) => {
                      setValues((prev) => ({ ...prev, [field.name]: event.target.value }));
                    }}
                  >
                    <option value="">{tr('Choose…', '请选择…')}</option>
                    {field.options.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    className="tv-mtpl-input"
                    type="text"
                    value={values[field.name] ?? ''}
                    onChange={(event) => {
                      setValues((prev) => ({ ...prev, [field.name]: event.target.value }));
                    }}
                  />
                )}
              </label>
            ))}
          </div>
        )}

        {preview !== null && (
          <>
            <div className="tv-mtpl-preview-head">
              <span className="tv-med-caps">{tr('Preview', '预览')}</span>
              {updating && <span className="tv-mtpl-updating">{tr('updating', '更新中')}</span>}
            </div>
            <div className="tv-mtpl-preview">{previewNodes()}</div>
          </>
        )}

        <div className="tv-mtpl-bar">
          <button type="button" className="tv-mtpl-copy" onClick={copy}>
            {tr('Copy', '复制')}
          </button>
        </div>
      </section>
    </div>
  );
}

/**
 * Import — a real drop zone instead of a native file input.
 * The format is inferred from the dropped file's extension and can be
 * overridden with quiet chips; results stay the honest full report
 * (imported / conflicts / skipped with reasons).
 */
import {
  espansoImport,
  ipcErrorCopy,
  snippetsImport,
  type ImportFormat,
  type ImportReport,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useRef, useState } from 'react';
import { PrefSection } from './pref-section';
import { SetRow } from './pref-kit';

type ToolFormat = ImportFormat | 'espanso';

const MAX_IMPORT_BYTES = 10 * 1024 * 1024;
const REPORT_LIST_LIMIT = 8;

const FORMATS: ReadonlyArray<{ id: ToolFormat; label: string; ext: string[] }> = [
  { id: 'markdown', label: 'Markdown', ext: ['md', 'markdown'] },
  { id: 'json', label: 'JSON', ext: ['json'] },
  { id: 'csv', label: 'CSV', ext: ['csv'] },
  { id: 'masscode', label: 'massCode', ext: [] },
  { id: 'copyq', label: 'CopyQ', ext: [] },
  { id: 'espanso', label: 'Espanso', ext: ['yml', 'yaml'] },
];

function inferFormat(name: string): ToolFormat | null {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  return FORMATS.find((format) => format.ext.includes(ext))?.id ?? null;
}

export function ImportPref() {
  const tr = useTr();
  const fileInput = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  const [format, setFormat] = useState<ToolFormat>('markdown');
  const [fileName, setFileName] = useState<string | null>(null);
  const [text, setText] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pick = async (file: File | undefined) => {
    setReport(null);
    setError(null);
    setFileName(null);
    setText(null);
    if (!file) return;
    if (file.size > MAX_IMPORT_BYTES) {
      setError(
        tr(
          'Your library was not changed — the file is larger than the 10 MB import limit.',
          '你的库未被更改——文件超过了 10 MB 的导入大小上限。',
        ),
      );
      return;
    }
    const inferred = inferFormat(file.name);
    if (inferred) setFormat(inferred);
    setFileName(file.name);
    setText(await file.text());
  };

  const run = async () => {
    if (text === null) return;
    setBusy(true);
    setError(null);
    try {
      const summary =
        format === 'espanso'
          ? await espansoImport(text).then((result) => ({
              imported: result.imported,
              conflicts: result.conflicts,
              skipped: result.skipped.map((skip) => ({ label: skip.trigger, reason: skip.reason })),
            }))
          : await snippetsImport(format, text);
      setReport(summary);
    } catch (caught) {
      const reason =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the file could not be imported', '文件无法导入');
      setError(tr(`Your library was not changed — ${reason}.`, `你的库未被更改——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  return (
    <PrefSection
      id="import"
      glyph="import"
      name={tr('Import', '导入')}
      sub="Markdown · JSON · CSV · Espanso +2"
    >
      <div
        className={`tvp-dropzone ${over ? 'over' : ''}`}
        onDragOver={(event) => {
          event.preventDefault();
          setOver(true);
        }}
        onDragLeave={() => setOver(false)}
        onDrop={(event) => {
          event.preventDefault();
          setOver(false);
          void pick(event.dataTransfer.files[0]);
        }}
      >
        <svg
          className="tvp-dz-icon"
          width="26"
          height="26"
          viewBox="0 0 20 20"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M10 3.5v9.5M6.2 9.6l3.8 3.9 3.8-3.9" />
          <path d="M4 16.5h12" />
        </svg>
        <div className="tvp-dz-title">
          {fileName
            ? tr(`Ready to import ${fileName}`, `已就绪:${fileName}`)
            : tr('Drop your snippets here', '把片段文件拖到这里')}
        </div>
        <div className="tvp-dz-fmts">Markdown · JSON · CSV · massCode · CopyQ · Espanso</div>
        {fileName ? (
          <button type="button" className="tvp-btn" disabled={busy} onClick={() => void run()}>
            {busy ? tr('Importing…', '导入中…') : tr('Import', '导入')}
          </button>
        ) : (
          <button
            type="button"
            className="tvp-btn quiet"
            onClick={() => fileInput.current?.click()}
          >
            {tr('Choose file…', '选择文件…')}
          </button>
        )}
        <input
          ref={fileInput}
          type="file"
          className="tvp-dz-file"
          accept=".md,.markdown,.json,.csv,.yml,.yaml,text/markdown,application/json,text/csv,text/yaml"
          aria-label={tr('Import file', '导入文件')}
          onChange={(event) => void pick(event.target.files?.[0])}
        />
      </div>

      {fileName && (
        <div
          className="tvp-fmt-chips"
          style={{ marginTop: 14 }}
          role="group"
          aria-label={tr('Import format', '导入格式')}
        >
          {FORMATS.map((option) => (
            <button
              key={option.id}
              type="button"
              className="tvp-chip"
              aria-pressed={option.id === format}
              onClick={() => {
                setFormat(option.id);
                setReport(null);
                setError(null);
              }}
            >
              {option.label}
            </button>
          ))}
        </div>
      )}

      {error && (
        <p className="tvp-note" role="status">
          {error}
        </p>
      )}
      {report && (
        <div role="status" style={{ marginTop: 14 }}>
          <SetRow
            title={tr(
              `Imported ${String(report.imported)} snippet${report.imported === 1 ? '' : 's'}.`,
              `已导入 ${String(report.imported)} 个片段。`,
            )}
            description={tr(
              `${String(report.conflicts.length)} trigger conflict${report.conflicts.length === 1 ? '' : 's'}, ${String(report.skipped.length)} skipped.`,
              `${String(report.conflicts.length)} 个触发词冲突,${String(report.skipped.length)} 条被跳过。`,
            )}
          />
          {report.conflicts.length > 0 && (
            <p className="tvp-note">
              {tr('Trigger already in use: ', '触发词已被占用:')}
              {report.conflicts.slice(0, REPORT_LIST_LIMIT).join(', ')}
              {report.conflicts.length > REPORT_LIST_LIMIT &&
                tr(
                  ` …and ${String(report.conflicts.length - REPORT_LIST_LIMIT)} more`,
                  ` …另有 ${String(report.conflicts.length - REPORT_LIST_LIMIT)} 条`,
                )}
            </p>
          )}
          {report.skipped.length > 0 && (
            <p className="tvp-note">
              {tr('Skipped: ', '已跳过:')}
              {report.skipped
                .slice(0, REPORT_LIST_LIMIT)
                .map((skip) => (skip.label ? `${skip.label} — ${skip.reason}` : skip.reason))
                .join(' · ')}
            </p>
          )}
        </div>
      )}
      <p className="tvp-note">
        {tr(
          'Your library is never changed until you confirm an import.',
          '在你确认导入之前,库不会被更改。',
        )}
      </p>
    </PrefSection>
  );
}

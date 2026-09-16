// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  backupExportFile,
  backupRestore,
  espansoImport,
  ipcErrorCopy,
  snippetsImport,
  type ImportFormat,
  type ImportReport,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useRef, useState } from 'react';
import { Choice } from '../../paper/choice';
import { GroupTitle, TextAction } from '../../paper/kit';
import { Actions, LineField, Said, SettingRow } from './settings-kit';

type ToolFormat = ImportFormat | 'espanso';

const MAX_IMPORT_BYTES = 10 * 1024 * 1024;
const REPORT_LIST_LIMIT = 8;
const MIN_PASSPHRASE = 8;

const FORMATS: ReadonlyArray<{ value: ToolFormat; label: string; ext: readonly string[] }> = [
  { value: 'markdown', label: 'Markdown', ext: ['md', 'markdown'] },
  { value: 'json', label: 'JSON', ext: ['json'] },
  { value: 'csv', label: 'CSV', ext: ['csv'] },
  { value: 'masscode', label: 'massCode', ext: [] },
  { value: 'copyq', label: 'CopyQ', ext: [] },
  { value: 'espanso', label: 'Espanso', ext: ['yml', 'yaml'] },
];

function inferFormat(name: string): ToolFormat | null {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  return FORMATS.find((format) => format.ext.includes(ext))?.value ?? null;
}

export function DataGroup() {
  const tr = useTr();
  return (
    <section className="tvs-group">
      <GroupTitle>{tr('Import & export', '导入导出')}</GroupTitle>
      <ImportRow />
      <ExportRow />
      <RestoreRow />
    </section>
  );
}

function ImportRow() {
  const tr = useTr();
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  const [format, setFormat] = useState<ToolFormat>('markdown');
  const [file, setFile] = useState<{ name: string; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pick = async (picked: File | undefined) => {
    setReport(null);
    setError(null);
    setFile(null);
    if (picked === undefined) return;
    if (picked.size > MAX_IMPORT_BYTES) {
      setError(
        tr(
          'Your library was not changed — the file is larger than the 10 MB import limit.',
          '库没有被改动——文件超过了 10 MB 的导入上限。',
        ),
      );
      return;
    }
    const inferred = inferFormat(picked.name);
    if (inferred !== null) setFormat(inferred);
    setFile({ name: picked.name, text: await picked.text() });
  };

  const run = async () => {
    if (file === null) return;
    setBusy(true);
    setError(null);
    try {
      setReport(
        format === 'espanso'
          ? await espansoImport(file.text).then((result) => ({
              imported: result.imported,
              conflicts: result.conflicts,
              skipped: result.skipped.map((skip) => ({ label: skip.trigger, reason: skip.reason })),
            }))
          : await snippetsImport(format, file.text),
      );
    } catch (caught) {
      const reason =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the file could not be imported', '文件导不进来');
      setError(tr(`Your library was not changed — ${reason}.`, `库没有被改动——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingRow label={tr('Import', '导入')}>
      <div
        className={over ? 'tvs-drop is-over' : 'tvs-drop'}
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
        <div className="tvs-inline">
          <span className="tvs-text is-meta">
            Markdown · JSON · CSV · massCode · CopyQ · Espanso
          </span>
          {file === null && (
            <TextAction onClick={() => input.current?.click()}>
              {tr('Choose a file…', '选择文件…')}
            </TextAction>
          )}
        </div>
      </div>
      <input
        ref={input}
        type="file"
        className="tvs-file"
        accept=".md,.markdown,.json,.csv,.yml,.yaml,text/markdown,application/json,text/csv,text/yaml"
        aria-label={tr('Import file', '导入文件')}
        onChange={(event) => void pick(event.target.files?.[0])}
      />
      {file !== null && (
        <>
          <p className="tvs-text is-mono">{file.name}</p>
          <Choice<ToolFormat>
            label={tr('Import format', '导入格式')}
            options={FORMATS}
            value={format}
            onChange={(next) => {
              setFormat(next);
              setReport(null);
              setError(null);
            }}
          />
        </>
      )}
      {file !== null && (
        <Actions>
          <TextAction primary disabled={busy} onClick={() => void run()}>
            {busy ? tr('Importing…', '正在导入…') : tr('Import', '导入')}
          </TextAction>
          <TextAction onClick={() => input.current?.click()}>
            {tr('Another file…', '换一个文件…')}
          </TextAction>
        </Actions>
      )}
      {error !== null && <Said>{error}</Said>}
      {report !== null && (
        <Said>
          {tr(
            `Imported ${String(report.imported)}. ${String(report.conflicts.length)} trigger clash${report.conflicts.length === 1 ? '' : 'es'}, ${String(report.skipped.length)} skipped.`,
            `导进来 ${String(report.imported)} 条。${String(report.conflicts.length)} 个触发词撞了,${String(report.skipped.length)} 条跳过。`,
          )}
          {report.conflicts.length > 0 && (
            <span className="tvs-said-list">
              {tr('Already in use: ', '已经被占用:')}
              {report.conflicts.slice(0, REPORT_LIST_LIMIT).join(', ')}
              {report.conflicts.length > REPORT_LIST_LIMIT &&
                tr(
                  ` and ${String(report.conflicts.length - REPORT_LIST_LIMIT)} more`,
                  ` 等另外 ${String(report.conflicts.length - REPORT_LIST_LIMIT)} 个`,
                )}
            </span>
          )}
          {report.skipped.length > 0 && (
            <span className="tvs-said-list">
              {tr('Skipped: ', '跳过的:')}
              {report.skipped
                .slice(0, REPORT_LIST_LIMIT)
                .map((skip) => (skip.label ? `${skip.label} — ${skip.reason}` : skip.reason))
                .join(' · ')}
            </span>
          )}
        </Said>
      )}
    </SettingRow>
  );
}

function ExportRow() {
  const tr = useTr();
  const [open, setOpen] = useState(false);
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setSaid(null);
    try {
      const path = await backupExportFile(passphrase);
      setPassphrase('');
      setSaid(tr(`Saved as ${path}.`, `存好了:${path}。`));
    } catch (caught) {
      const reason =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the backup could not be written', '备份写不出来');
      setSaid(tr(`Your data is untouched — ${reason}.`, `你的数据没动——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  const short = passphrase.length > 0 && passphrase.length < MIN_PASSPHRASE;

  return (
    <SettingRow label={tr('Export', '导出')}>
      <p className="tvs-text is-value">
        {tr(
          'An encrypted backup · snippets, collections, history and the vault',
          '加密备份 · 片段、集合、历史和保险库',
        )}
      </p>
      {open ? (
        <div className="tvs-form">
          <LineField
            label={tr('Backup passphrase', '备份密码短语')}
            type="password"
            autoComplete="new-password"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
          />
          {short && (
            <p className="tvs-text is-meta">{tr('At least 8 characters.', '至少 8 个字符。')}</p>
          )}
          <Actions>
            <TextAction
              primary
              disabled={busy || passphrase.length < MIN_PASSPHRASE}
              onClick={() => void run()}
            >
              {busy ? tr('Exporting…', '正在导出…') : tr('Export', '导出')}
            </TextAction>
            <TextAction onClick={() => setOpen(false)}>{tr('Not now', '先不')}</TextAction>
          </Actions>
        </div>
      ) : (
        <Actions>
          <TextAction primary onClick={() => setOpen(true)}>
            {tr('Export all snippets', '导出全部片段')}
          </TextAction>
        </Actions>
      )}
      {said !== null && <Said>{said}</Said>}
    </SettingRow>
  );
}

function RestoreRow() {
  const tr = useTr();
  const input = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<{ name: string; text: string } | null>(null);
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);

  const run = async () => {
    if (file === null) return;
    setBusy(true);
    setSaid(null);
    try {
      const report = await backupRestore(passphrase, file.text);
      setPassphrase('');
      setSaid(
        tr(
          `Restored ${String(report.snippets)} snippets and ${String(report.folders)} collections.`,
          `恢复了 ${String(report.snippets)} 条片段、${String(report.folders)} 个集合。`,
        ),
      );
    } catch (caught) {
      const reason =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the backup could not be read', '备份读不出来');
      setSaid(tr(`Nothing was changed — ${reason}.`, `什么都没改——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingRow label={tr('Restore', '从备份恢复')}>
      <input
        ref={input}
        type="file"
        className="tvs-file"
        accept=".typvia-backup,.json,application/json"
        aria-label={tr('Backup file', '备份文件')}
        onChange={(event) => {
          const picked = event.target.files?.[0];
          if (picked === undefined) return;
          void picked.text().then((text) => setFile({ name: picked.name, text }));
        }}
      />
      {file !== null && (
        <div className="tvs-form">
          <p className="tvs-text is-mono">{file.name}</p>
          <LineField
            label={tr('Passphrase', '密码短语')}
            type="password"
            autoComplete="off"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
          />
        </div>
      )}
      <Actions>
        {file !== null && (
          <TextAction primary disabled={busy || passphrase === ''} onClick={() => void run()}>
            {busy ? tr('Restoring…', '正在恢复…') : tr('Restore', '恢复')}
          </TextAction>
        )}
        <TextAction onClick={() => input.current?.click()}>
          {file === null
            ? tr('Choose a backup file…', '选择备份文件…')
            : tr('Another file…', '换一个文件…')}
        </TextAction>
      </Actions>
      {said !== null && <Said>{said}</Said>}
    </SettingRow>
  );
}

/**
 * Data & privacy — backup, restore and app visibility folded into one
 * section. Export reveals its passphrase field on demand; restore hides
 * until asked; the app-rules editor keeps its proven flow, embedded inside a
 * fold until its own redesign pass.
 */
import { appRuleList, backupExportFile, backupRestore, ipcErrorCopy } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { AppRulesSection } from '../app-rules-section';
import { PrefSection } from './pref-section';
import { Chip, Fold, FoldRow, KV, SetLabel, SetRow } from './pref-kit';

export function DataPref() {
  const tr = useTr();
  const [ruleCount, setRuleCount] = useState<number | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [rulesOpen, setRulesOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  useEffect(() => {
    appRuleList(200, 0)
      .then((rules) => setRuleCount(rules.length))
      .catch(() => setRuleCount(null));
  }, [rulesOpen]);

  return (
    <PrefSection
      id="data"
      glyph="data"
      name={tr('Data & privacy', '数据与隐私')}
      sub={
        ruleCount === null || ruleCount === 0
          ? tr('Encrypted backup · every app sees every snippet', '加密备份 · 所有应用可见全部片段')
          : tr(
              `Encrypted backup · ${String(ruleCount)} app rule${ruleCount === 1 ? '' : 's'}`,
              `加密备份 · ${String(ruleCount)} 条应用规则`,
            )
      }
    >
      <SetLabel>{tr('Backup', '备份')}</SetLabel>
      <SetRow
        title={tr('Encrypted backup', '加密备份')}
        description={tr(
          'Everything — snippets, folders, history, vault — sealed with a passphrase.',
          '片段、文件夹、历史与保险库,整体以密码短语加密。',
        )}
        actions={
          <Chip cta onClick={() => setExportOpen((open) => !open)}>
            {tr('Export backup', '导出备份')}
          </Chip>
        }
      />
      <Fold open={exportOpen}>
        <ExportForm />
      </Fold>

      <FoldRow
        label={tr('Restore from a backup', '从备份恢复')}
        open={restoreOpen}
        onToggle={() => setRestoreOpen((open) => !open)}
      />
      <Fold open={restoreOpen}>
        <RestoreForm />
      </Fold>

      <SetLabel>{tr('App visibility', '应用可见性')}</SetLabel>
      <SetRow
        title={
          ruleCount === null || ruleCount === 0
            ? tr('Every app can access every snippet.', '现在,每个应用都能使用全部片段。')
            : tr(
                `${String(ruleCount)} custom rule${ruleCount === 1 ? '' : 's'}`,
                `${String(ruleCount)} 条自定义规则`,
              )
        }
        description={tr('Limit which snippets each app can see.', '限定每个应用能看到哪些片段。')}
        actions={
          <Chip onClick={() => setRulesOpen((open) => !open)}>
            {ruleCount ? tr('Manage', '管理') : tr('Add a rule', '添加规则')}
          </Chip>
        }
      />
      <Fold open={rulesOpen}>
        <div className="tvp-legacy">
          <AppRulesSection />
        </div>
      </Fold>

      <FoldRow
        label={tr('Advanced', '高级')}
        open={advancedOpen}
        onToggle={() => setAdvancedOpen((open) => !open)}
      />
      <Fold open={advancedOpen}>
        <KV
          rows={[
            [tr('Database', '数据库'), tr('local SQLite in app data', '本机 app data 内的 SQLite')],
            [
              tr('AI egress log', 'AI 出站日志'),
              tr('always on — full retention', '始终记录——全量保留'),
            ],
            [
              tr('Backup format', '备份格式'),
              tr(
                'one encrypted file; wrong passphrase restores nothing',
                '单一加密文件;密码错误不会恢复任何内容',
              ),
            ],
          ]}
        />
      </Fold>
    </PrefSection>
  );
}

function ExportForm() {
  const tr = useTr();
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [savedAs, setSavedAs] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setError(null);
    setSavedAs(null);
    try {
      setSavedAs(await backupExportFile(passphrase));
      setPassphrase('');
    } catch (caught) {
      setError(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the backup could not be written', '备份无法写入'),
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tvp-fields" style={{ marginTop: 4 }}>
      <div className="tvp-field">
        <label htmlFor="tvp-backup-pass">{tr('Backup passphrase', '备份密码短语')}</label>
        <input
          id="tvp-backup-pass"
          type="password"
          value={passphrase}
          autoComplete="new-password"
          onChange={(event) => setPassphrase(event.target.value)}
        />
      </div>
      <div>
        <button
          type="button"
          className="tvp-btn"
          disabled={busy || passphrase.length < 8}
          onClick={() => void run()}
        >
          {busy ? tr('Exporting…', '导出中…') : tr('Export', '导出')}
        </button>
        {passphrase.length > 0 && passphrase.length < 8 && (
          <span className="tvp-note" style={{ marginLeft: 10 }}>
            {tr('At least 8 characters.', '至少 8 个字符。')}
          </span>
        )}
      </div>
      {savedAs && (
        <p className="tvp-note" role="status">
          {tr(`Saved as ${savedAs}.`, `已保存为 ${savedAs}。`)}
        </p>
      )}
      {error && (
        <p className="tvp-note" role="status">
          {tr(`Your data is untouched — ${error}.`, `你的数据完好——${error}。`)}
        </p>
      )}
    </div>
  );
}

function RestoreForm() {
  const tr = useTr();
  const fileInput = useRef<HTMLInputElement>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [content, setContent] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    if (content === null) return;
    setBusy(true);
    setError(null);
    try {
      const report = await backupRestore(passphrase, content);
      setDone(
        tr(
          `Restored ${String(report.snippets)} snippets, ${String(report.folders)} folders, ${String(report.tags)} tags.`,
          `已恢复 ${String(report.snippets)} 个片段、${String(report.folders)} 个文件夹、${String(report.tags)} 个标签。`,
        ),
      );
    } catch (caught) {
      const reason =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the backup could not be read', '备份无法读取');
      setError(tr(`Nothing was changed — ${reason}.`, `未做任何更改——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tvp-fields" style={{ marginTop: 4 }}>
      <div>
        <button type="button" className="tvp-btn quiet" onClick={() => fileInput.current?.click()}>
          {fileName ?? tr('Choose backup file…', '选择备份文件…')}
        </button>
        <input
          ref={fileInput}
          type="file"
          className="tvp-dz-file"
          accept=".typvia-backup,.json,application/json"
          aria-label={tr('Backup file', '备份文件')}
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (!file) return;
            setFileName(file.name);
            void file.text().then(setContent);
          }}
        />
      </div>
      <div className="tvp-field">
        <label htmlFor="tvp-restore-pass">{tr('Passphrase', '密码短语')}</label>
        <input
          id="tvp-restore-pass"
          type="password"
          value={passphrase}
          autoComplete="off"
          onChange={(event) => setPassphrase(event.target.value)}
        />
      </div>
      <div>
        <button
          type="button"
          className="tvp-btn"
          disabled={busy || content === null || passphrase === ''}
          onClick={() => void run()}
        >
          {busy ? tr('Restoring…', '恢复中…') : tr('Restore', '恢复')}
        </button>
      </div>
      {done && (
        <p className="tvp-note" role="status">
          {done}
        </p>
      )}
      {error && (
        <p className="tvp-note" role="status">
          {error}
        </p>
      )}
    </div>
  );
}

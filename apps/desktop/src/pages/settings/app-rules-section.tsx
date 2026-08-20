import {
  type AppRule,
  appRuleCreate,
  appRuleDelete,
  appRuleList,
  type AppRuleType,
  appRuleUpdate,
  ipcErrorCopy,
  searchLibrary,
  type Snippet,
} from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';

/**
 * Settings — App rules group (no dedicated design exists; built in the
 * settings language established by the Espanso block). A rule binds one
 * snippet to one app: show it only there, hide it there, or refuse sensitive
 * insert there. Rules filter the panel and the insert path — the vault, not
 * rules, stays the security boundary.
 */

/** One page covers realistic rule counts; the cap is stated when reached. */
const RULES_PAGE = 200;

/** Snippet picker shortlist size. */
const PICKER_LIMIT = 8;

interface RuleTypeOption {
  id: AppRuleType;
  label: string;
  hint: string;
}

/** The three effects a rule can have, worded in the active language. */
function ruleTypeOptions(tr: Tr): RuleTypeOption[] {
  return [
    {
      id: 'show_only',
      label: tr('Show only here', '仅在此应用显示'),
      hint: tr(
        'The snippet appears in this app and nowhere else.',
        '该片段只出现在这个应用中，其他任何地方都不会出现。',
      ),
    },
    {
      id: 'disable',
      label: tr('Hide here', '在此应用隐藏'),
      hint: tr('The snippet never appears in this app.', '该片段永远不会出现在这个应用中。'),
    },
    {
      id: 'deny_sensitive_injection',
      label: tr('No sensitive insert', '拦截敏感插入'),
      hint: tr(
        'Sensitive insert into this app is refused; copy stays available.',
        '拒绝向这个应用插入敏感内容；复制仍然可用。',
      ),
    },
  ];
}

function ruleLabel(ruleType: string, tr: Tr): string {
  return ruleTypeOptions(tr).find((option) => option.id === ruleType)?.label ?? ruleType;
}

export function AppRulesSection() {
  const tr = useTr();
  // `null` while the first load is in flight (loading state, no spinner).
  const [rules, setRules] = useState<AppRule[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);

  const reload = useCallback(() => {
    appRuleList(RULES_PAGE, 0)
      .then((rows) => {
        setRules(rows);
        setLoadFailed(false);
      })
      .catch(() => {
        setLoadFailed(true);
      });
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const changed = () => {
    setEditing(null);
    reload();
  };

  return (
    <section className="tv-settings-group" aria-labelledby="app-rules-heading">
      <div className="tv-settings-group-head">
        <h2 id="app-rules-heading" className="tv-settings-group-title">
          {tr('App rules', '应用规则')}
        </h2>
      </div>
      <p className="tv-settings-lead">
        {tr('Decide which apps see which snippets.', '决定哪些应用能看到哪些片段。')}
      </p>

      {loadFailed && (
        <p className="tv-settings-note" role="status">
          {tr(
            'Your rules are intact — the list could not be loaded. Reopen Settings.',
            '你的规则完好——列表暂时无法加载。请重新打开设置。',
          )}
        </p>
      )}
      {rules === null && !loadFailed && (
        <p className="tv-settings-note">{tr('Loading rules…', '正在加载规则…')}</p>
      )}
      {rules !== null && rules.length === 0 && (
        <p className="tv-settings-note">{tr('No rules yet.', '还没有规则。')}</p>
      )}

      {rules !== null && rules.length > 0 && (
        <ul className="tv-settings-rules" aria-label={tr('App rules', '应用规则')}>
          {rules.map((rule) =>
            editing === rule.id ? (
              <li key={rule.id} className="tv-settings-rule">
                <RuleEditor rule={rule} onDone={changed} onCancel={() => setEditing(null)} />
              </li>
            ) : (
              <RuleRow
                key={rule.id}
                rule={rule}
                onEdit={() => setEditing(rule.id)}
                onRemoved={changed}
              />
            ),
          )}
        </ul>
      )}
      {rules !== null && rules.length >= RULES_PAGE && (
        <p className="tv-settings-count">
          {tr(`Showing the first ${RULES_PAGE} rules.`, `仅显示前 ${RULES_PAGE} 条规则。`)}
        </p>
      )}

      <AddRuleForm onAdded={changed} />
    </section>
  );
}

function RuleRow({
  rule,
  onEdit,
  onRemoved,
}: {
  rule: AppRule;
  onEdit: () => void;
  onRemoved: () => void;
}) {
  const tr = useTr();
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const remove = () => {
    appRuleDelete(rule.id)
      .then(onRemoved)
      .catch(() => {
        setConfirming(false);
        setError(
          tr(
            'The rule is still there — removing it failed. Try again.',
            '规则仍然保留——删除失败。请重试。',
          ),
        );
      });
  };

  return (
    <li className="tv-settings-rule">
      <span className="tv-settings-rule-title">{rule.snippetTitle}</span>
      <code className="tv-settings-rule-app">{rule.appIdentifier}</code>
      <span className="tv-settings-rule-kind">{ruleLabel(rule.ruleType, tr)}</span>
      <span className="tv-settings-rule-actions">
        <button type="button" className="tv-settings-ghost" onClick={onEdit}>
          {tr('Edit', '编辑')}
        </button>
        {confirming ? (
          <button type="button" className="tv-settings-danger" onClick={remove}>
            {tr('Remove rule', '移除规则')}
          </button>
        ) : (
          <button type="button" className="tv-settings-danger" onClick={() => setConfirming(true)}>
            {tr('Remove', '移除')}
          </button>
        )}
      </span>
      {error !== null && (
        <span className="tv-settings-note" role="status">
          {error}
        </span>
      )}
    </li>
  );
}

function RuleEditor({
  rule,
  onDone,
  onCancel,
}: {
  rule: AppRule;
  onDone: () => void;
  onCancel: () => void;
}) {
  const tr = useTr();
  const [appId, setAppId] = useState(rule.appIdentifier);
  const [ruleType, setRuleType] = useState<AppRuleType>(rule.ruleType);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await appRuleUpdate(rule.id, { appIdentifier: appId, ruleType });
      onDone();
    } catch (caught) {
      const reason =
        caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('saving failed', '保存失败');
      setError(tr(`The rule was not changed — ${reason}.`, `规则未被更改——${reason}。`));
      setBusy(false);
    }
  };

  return (
    <div className="tv-settings-rule-form">
      <span className="tv-settings-rule-title">{rule.snippetTitle}</span>
      <input
        className="tv-settings-rule-input"
        value={appId}
        onChange={(event) => setAppId(event.target.value)}
        spellCheck={false}
        aria-label={tr('App identifier', '应用标识符')}
      />
      <RuleTypePicker value={ruleType} onChange={setRuleType} />
      <span className="tv-settings-rule-actions">
        <button
          type="button"
          className="tv-settings-ghost"
          disabled={busy || appId.trim() === ''}
          onClick={() => void save()}
        >
          {busy ? tr('Saving…', '保存中…') : tr('Save', '保存')}
        </button>
        <button type="button" className="tv-settings-ghost" onClick={onCancel}>
          {tr('Cancel', '取消')}
        </button>
      </span>
      {error !== null && (
        <span className="tv-settings-note" role="status">
          {error}
        </span>
      )}
    </div>
  );
}

function RuleTypePicker({
  value,
  onChange,
}: {
  value: AppRuleType;
  onChange: (next: AppRuleType) => void;
}) {
  const tr = useTr();
  return (
    <div className="tv-settings-formats" role="group" aria-label={tr('Rule effect', '规则效果')}>
      {ruleTypeOptions(tr).map((option) => (
        <button
          key={option.id}
          type="button"
          className="tv-settings-ghost"
          aria-pressed={option.id === value}
          onClick={() => onChange(option.id)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function AddRuleForm({ onAdded }: { onAdded: () => void }) {
  const tr = useTr();
  const [snippet, setSnippet] = useState<Snippet | null>(null);
  const [pickQuery, setPickQuery] = useState('');
  const [matches, setMatches] = useState<Snippet[]>([]);
  const [appId, setAppId] = useState('');
  const [ruleType, setRuleType] = useState<AppRuleType>('disable');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const search = (value: string) => {
    setPickQuery(value);
    if (value.trim() === '') {
      setMatches([]);
      return;
    }
    searchLibrary(value, PICKER_LIMIT)
      .then(setMatches)
      .catch(() => setMatches([]));
  };

  const add = async () => {
    if (snippet === null) return;
    setBusy(true);
    setError(null);
    try {
      await appRuleCreate({ snippetId: snippet.id, appIdentifier: appId, ruleType });
      setSnippet(null);
      setPickQuery('');
      setMatches([]);
      setAppId('');
      onAdded();
    } catch (caught) {
      const reason =
        caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('adding failed', '添加失败');
      setError(tr(`No rule was added — ${reason}.`, `未添加规则——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tv-settings-row tv-settings-block">
      <div className="tv-settings-row-label">{tr('Add a rule', '添加规则')}</div>

      {snippet === null ? (
        <>
          <input
            className="tv-settings-rule-input"
            value={pickQuery}
            onChange={(event) => search(event.target.value)}
            spellCheck={false}
            aria-label={tr('Find a snippet', '查找片段')}
            placeholder={tr('Find a snippet…', '查找片段…')}
          />
          {matches.length > 0 && (
            <ul className="tv-settings-picker" aria-label={tr('Matching snippets', '匹配的片段')}>
              {matches.map((match) => (
                <li key={match.id}>
                  <button
                    type="button"
                    className="tv-settings-ghost"
                    onClick={() => {
                      setSnippet(match);
                      setMatches([]);
                    }}
                  >
                    {match.title}
                  </button>
                </li>
              ))}
            </ul>
          )}
          {pickQuery.trim() !== '' && matches.length === 0 && (
            <p className="tv-settings-note">{tr('No matching snippets.', '没有匹配的片段。')}</p>
          )}
        </>
      ) : (
        <div className="tv-settings-command">
          <span className="tv-settings-rule-title">{snippet.title}</span>
          <button type="button" className="tv-settings-ghost" onClick={() => setSnippet(null)}>
            {tr('Change', '更换')}
          </button>
        </div>
      )}

      <input
        className="tv-settings-rule-input"
        value={appId}
        onChange={(event) => setAppId(event.target.value)}
        spellCheck={false}
        aria-label={tr('App identifier', '应用标识符')}
        placeholder="com.google.Chrome"
      />
      <RuleTypePicker value={ruleType} onChange={setRuleType} />
      <div className="tv-settings-command">
        <button
          type="button"
          className="tv-settings-ghost"
          disabled={busy || snippet === null || appId.trim() === ''}
          onClick={() => void add()}
        >
          {busy ? tr('Adding…', '添加中…') : tr('Add rule', '添加规则')}
        </button>
      </div>
      <p className="tv-settings-row-meta">
        {ruleTypeOptions(tr).find((option) => option.id === ruleType)?.hint}
      </p>
      {error !== null && (
        <p className="tv-settings-note" role="status">
          {error}
        </p>
      )}
    </div>
  );
}

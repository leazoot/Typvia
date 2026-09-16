// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  appRuleCreate,
  appRuleDelete,
  appRuleList,
  appRuleUpdate,
  ipcErrorCopy,
  searchLibrary,
  type AppRule,
  type AppRuleType,
  type Snippet,
} from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';
import { Choice } from '../../paper/choice';
import { GroupTitle, TextAction } from '../../paper/kit';
import { useUndo } from '../../workspace/undo';
import { Actions, DangerAction, LineField, Said, SettingRow } from './settings-kit';

/** One page covers realistic rule counts; the cap is said when reached. */
const RULES_PAGE = 200;
const PICKER_LIMIT = 8;

function ruleTypes(tr: Tr): { value: AppRuleType; label: string }[] {
  return [
    {
      value: 'show_only',
      label: tr('Show only here', '仅在此应用显示'),
    },
    {
      value: 'disable',
      label: tr('Hide here', '在此应用隐藏'),
    },
    {
      value: 'deny_sensitive_injection',
      label: tr('No sensitive insert', '拦截敏感插入'),
    },
  ];
}

export function AppRulesGroup({ total }: { total: number | null }) {
  const tr = useTr();
  const { offer } = useUndo();
  const [rules, setRules] = useState<AppRule[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [said, setSaid] = useState<string | null>(null);

  const reload = useCallback(() => {
    appRuleList(RULES_PAGE, 0)
      .then((rows) => {
        setRules(rows);
        setLoadFailed(false);
      })
      .catch(() => setLoadFailed(true));
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const remove = (rule: AppRule) => {
    setSaid(null);
    appRuleDelete(rule.id)
      .then(() => {
        reload();
        offer({
          title: tr(
            `Removed the rule for ${rule.appIdentifier}.`,
            `已删掉 ${rule.appIdentifier} 的规则。`,
          ),
          body: tr('⌘Z puts it back.', '⌘Z 撤回。'),
          failure: tr('The rule could not be put back.', '规则没能加回来。'),
          undo: () =>
            appRuleCreate({
              snippetId: rule.snippetId,
              appIdentifier: rule.appIdentifier,
              ruleType: rule.ruleType,
            }).then(reload),
        });
      })
      .catch(() =>
        setSaid(
          tr(
            'The rule is still there — removing it failed. Try again.',
            '规则还在——没删掉。再试一次。',
          ),
        ),
      );
  };

  const types = ruleTypes(tr);
  const count = rules?.length ?? 0;

  return (
    <section className="tvs-group">
      <GroupTitle>{tr('Per-app visibility', '应用可见性')}</GroupTitle>
      <SettingRow label={tr('As it stands', '现在的样子')}>
        {loadFailed ? (
          <Said>
            {tr(
              'Your rules are intact — the list could not be read. Reopen Settings.',
              '规则都在——只是列表暂时读不出来。重新打开设置试试。',
            )}
          </Said>
        ) : rules === null ? null : count === 0 ? (
          <>
            <p className="tvs-lead">
              {total === null
                ? tr('Every app can use every snippet.', '现在,每个应用都能用全部片段。')
                : tr(
                    `Every app can use all ${String(total)} snippets.`,
                    `现在,每个应用都能用全部 ${String(total)} 条。`,
                  )}
            </p>
          </>
        ) : (
          <>
            <p className="tvs-text is-value">
              {tr(
                `${String(count)} rule${count === 1 ? '' : 's'} at work; every other snippet is open to every app.`,
                `有 ${String(count)} 条规则在起作用;其余的片段每个应用都能用。`,
              )}
            </p>
          </>
        )}
      </SettingRow>
      {rules !== null && count > 0 && (
        <SettingRow
          label={tr(`${String(count)} rule${count === 1 ? '' : 's'}`, `${String(count)} 条规则`)}
        >
          <ul className="tvs-rules" aria-label={tr('App rules', '应用规则')}>
            {rules.map((rule) =>
              editing === rule.id ? (
                <li key={rule.id}>
                  <RuleEditor
                    rule={rule}
                    onDone={() => {
                      setEditing(null);
                      reload();
                    }}
                    onCancel={() => setEditing(null)}
                  />
                </li>
              ) : (
                <li key={rule.id} className="tvs-rule">
                  <span className="tvs-rule-app">{rule.appIdentifier}</span>
                  <span className="tvs-rule-kind">
                    {types.find((type) => type.value === rule.ruleType)?.label ?? rule.ruleType}
                  </span>
                  <span className="tvs-rule-what">{rule.snippetTitle}</span>
                  <span className="tvs-spacer" />
                  <TextAction onClick={() => setEditing(rule.id)}>{tr('Change', '改')}</TextAction>
                  <DangerAction onClick={() => remove(rule)}>{tr('Remove', '删')}</DangerAction>
                </li>
              ),
            )}
          </ul>
          {count >= RULES_PAGE && (
            <p className="tvs-text is-meta">
              {tr(
                `Showing the first ${String(RULES_PAGE)} rules.`,
                `只列出前 ${String(RULES_PAGE)} 条。`,
              )}
            </p>
          )}
          {said !== null && <Said>{said}</Said>}
        </SettingRow>
      )}
      {adding ? (
        <AddRule
          onAdded={() => {
            setAdding(false);
            reload();
          }}
          onCancel={() => setAdding(false)}
        />
      ) : (
        rules !== null && (
          <SettingRow>
            <TextAction primary onClick={() => setAdding(true)}>
              {count === 0
                ? tr('Add the first rule', '添加第一条规则')
                : tr('Add a rule', '添加规则')}
            </TextAction>
          </SettingRow>
        )
      )}
    </section>
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
  const types = ruleTypes(tr);

  const save = () => {
    setBusy(true);
    setError(null);
    appRuleUpdate(rule.id, { appIdentifier: appId.trim(), ruleType })
      .then(onDone)
      .catch((caught: unknown) => {
        const reason =
          caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('saving failed', '没存上');
        setError(tr(`The rule was not changed — ${reason}.`, `规则没有改动——${reason}。`));
        setBusy(false);
      });
  };

  return (
    <div className="tvs-form">
      <p className="tvs-text is-value">{rule.snippetTitle}</p>
      <LineField
        label={tr('App identifier', '应用标识符')}
        mono
        value={appId}
        onChange={(event) => setAppId(event.target.value)}
      />
      <Choice
        label={tr('Rule effect', '规则效果')}
        options={types}
        value={ruleType}
        onChange={setRuleType}
      />
      {error !== null && <Said>{error}</Said>}
      <Actions>
        <TextAction primary disabled={busy || appId.trim() === ''} onClick={save}>
          {tr('Save', '保存')}
        </TextAction>
        <TextAction onClick={onCancel}>{tr('Cancel', '取消')}</TextAction>
      </Actions>
    </div>
  );
}

function AddRule({ onAdded, onCancel }: { onAdded: () => void; onCancel: () => void }) {
  const tr = useTr();
  const [snippet, setSnippet] = useState<Snippet | null>(null);
  const [query, setQuery] = useState('');
  const [matches, setMatches] = useState<Snippet[]>([]);
  const [appId, setAppId] = useState('');
  const [ruleType, setRuleType] = useState<AppRuleType>('disable');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const types = ruleTypes(tr);

  const find = (value: string) => {
    setQuery(value);
    if (value.trim() === '') {
      setMatches([]);
      return;
    }
    searchLibrary(value, PICKER_LIMIT)
      .then(setMatches)
      .catch(() => setMatches([]));
  };

  const add = () => {
    if (snippet === null) return;
    setBusy(true);
    setError(null);
    appRuleCreate({ snippetId: snippet.id, appIdentifier: appId.trim(), ruleType })
      .then(onAdded)
      .catch((caught: unknown) => {
        const reason =
          caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('adding failed', '没加上');
        setError(tr(`No rule was added — ${reason}.`, `没有加上规则——${reason}。`));
        setBusy(false);
      });
  };

  return (
    <>
      <SettingRow label={tr('Snippet', '片段')} narrow>
        {snippet === null ? (
          <>
            <LineField
              label={tr('Find a snippet', '找一条片段')}
              value={query}
              placeholder={tr('Type a few words…', '打几个字…')}
              onChange={(event) => find(event.target.value)}
            />
            {matches.length > 0 && (
              <ul className="tvs-picks" aria-label={tr('Matching snippets', '找到的片段')}>
                {matches.map((match) => (
                  <li key={match.id}>
                    <button type="button" className="tvs-pick" onClick={() => setSnippet(match)}>
                      {match.title}
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {query.trim() !== '' && matches.length === 0 && (
              <p className="tvs-text is-meta">
                {tr('No snippet matches.', '没有找到这样的片段。')}
              </p>
            )}
          </>
        ) : (
          <div className="tvs-inline">
            <span className="tvs-text is-value">{snippet.title}</span>
            <TextAction onClick={() => setSnippet(null)}>{tr('Pick another', '换一条')}</TextAction>
          </div>
        )}
      </SettingRow>
      <SettingRow label={tr('App', '应用')} narrow>
        <LineField
          label={tr('App identifier', '应用标识符')}
          mono
          value={appId}
          placeholder="com.google.Chrome"
          onChange={(event) => setAppId(event.target.value)}
        />
      </SettingRow>
      <SettingRow label={tr('Rule', '规则')}>
        <Choice
          label={tr('Rule effect', '规则效果')}
          options={types}
          value={ruleType}
          onChange={setRuleType}
        />
        {error !== null && <Said>{error}</Said>}
        <Actions>
          <TextAction
            primary
            disabled={busy || snippet === null || appId.trim() === ''}
            onClick={add}
          >
            {tr('Add the rule', '加上规则')}
          </TextAction>
          <TextAction onClick={onCancel}>{tr('Cancel', '取消')}</TextAction>
        </Actions>
      </SettingRow>
    </>
  );
}

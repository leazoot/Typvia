// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  aiApiKeyClear,
  aiApiKeySet,
  aiCheckConnectivity,
  aiProviderDelete,
  aiProviderList,
  aiProviderSave,
  ipcErrorCopy,
  type AiProvider,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { OLLAMA_KIND, PROVIDER_KINDS, kindLabel } from '@typvia/ui/ai';
import { useCallback, useEffect, useState } from 'react';
import { Choice } from '../../paper/choice';
import { GroupTitle, TextAction } from '../../paper/kit';
import { Actions, DangerAction, LineField, Said, SettingRow, State } from './settings-kit';

export function ProviderGroup() {
  const tr = useTr();
  const [providers, setProviders] = useState<AiProvider[] | null>(null);
  // 'add', a provider id being edited, or null when nothing is open.
  const [editing, setEditing] = useState<string | null>(null);
  const [probe, setProbe] = useState<string | null>(null);

  const refresh = useCallback(() => {
    aiProviderList()
      .then(setProviders)
      .catch(() => setProviders([]));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const test = (provider: AiProvider) => {
    setProbe(tr('Testing…', '正在试…'));
    aiCheckConnectivity(provider.id)
      .then((result) =>
        setProbe(
          result.modelAvailable === false
            ? tr(
                `Reachable — but the model “${provider.model}” is not listed.`,
                `连得上——但没找到模型「${provider.model}」。`,
              )
            : tr('Connected.', '连得上。'),
        ),
      )
      .catch((caught: unknown) =>
        setProbe(
          caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('Unreachable.', '连不上。'),
        ),
      );
  };

  const remove = (provider: AiProvider) => {
    aiProviderDelete(provider.id)
      .catch(() => undefined)
      .finally(refresh);
  };

  const current = providers?.[0] ?? null;
  const others = providers?.slice(1) ?? [];

  const saved = () => {
    setEditing(null);
    refresh();
  };

  return (
    <section className="tvs-group">
      <GroupTitle
        trailing={
          editing === null &&
          current !== null && (
            <button type="button" className="tvs-group-action" onClick={() => setEditing('add')}>
              {tr('Add another', '再接一个')}
            </button>
          )
        }
      >
        {tr('AI provider', '大模型')}
      </GroupTitle>
      {editing !== null ? (
        <ProviderForm
          key={editing}
          provider={providers?.find((provider) => provider.id === editing) ?? null}
          onCancel={() => setEditing(null)}
          onSaved={saved}
        />
      ) : current === null ? (
        <SettingRow label={tr('Provider', '服务方')}>
          {providers !== null && (
            <div className="tvs-inline">
              <State kind="idle">{tr('None connected', '还没接')}</State>
              <TextAction primary onClick={() => setEditing('add')}>
                {tr('Connect a provider', '接一个服务方')}
              </TextAction>
            </div>
          )}
        </SettingRow>
      ) : (
        <>
          <SettingRow label={tr('Provider', '服务方')}>
            <div className="tvs-inline">
              <span className="tvs-text is-value">
                {current.name} · {kindLabel(current.kind, tr)}
              </span>
              <TextAction onClick={() => test(current)}>
                {tr('Test the connection', '试一下连接')}
              </TextAction>
              <TextAction onClick={() => setEditing(current.id)}>{tr('Change', '改')}</TextAction>
              <DangerAction onClick={() => remove(current)}>{tr('Remove', '删')}</DangerAction>
            </div>
            {probe !== null && <Said>{probe}</Said>}
          </SettingRow>
          <SettingRow label={tr('Model', '模型')}>
            <p className="tvs-text is-mono">{current.model}</p>
            <p className="tvs-text is-meta">
              {current.baseUrl === ''
                ? tr('Address: the default for this kind.', '地址:这一类的默认地址。')
                : current.baseUrl}
            </p>
          </SettingRow>
          <SettingRow label="API Key" narrow>
            <div className="tvs-line is-static">
              <span className={current.hasApiKey ? 'tvs-line-text' : 'tvs-line-text is-faint'}>
                {current.hasApiKey
                  ? tr('Kept in the system keychain', '已存在系统钥匙串里')
                  : tr('Not needed — not a word leaves', '不需要 —— 一个字都不出去')}
              </span>
            </div>
          </SettingRow>
          {others.map((provider) => (
            <SettingRow key={provider.id} label={tr('Also connected', '另外接了')}>
              <div className="tvs-inline">
                <span className="tvs-text is-value">
                  {provider.name} · {kindLabel(provider.kind, tr)} · {provider.model}
                </span>
                <span className="tvs-spacer" />
                <TextAction onClick={() => setEditing(provider.id)}>
                  {tr('Change', '改')}
                </TextAction>
                <DangerAction onClick={() => remove(provider)}>{tr('Remove', '删')}</DangerAction>
              </div>
            </SettingRow>
          ))}
        </>
      )}
    </section>
  );
}

function ProviderForm({
  provider,
  onCancel,
  onSaved,
}: {
  provider: AiProvider | null;
  onCancel: () => void;
  onSaved: () => void;
}) {
  const tr = useTr();
  const [kindId, setKindId] = useState(provider?.kind ?? OLLAMA_KIND.id);
  const kind = PROVIDER_KINDS.find((option) => option.id === kindId) ?? OLLAMA_KIND;
  const [name, setName] = useState(provider?.name ?? '');
  const [model, setModel] = useState(provider?.model ?? '');
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? '');
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const missingKey = kind.needsKey && provider === null && key.trim() === '';
  const missingUrl = !kind.hasDefault && baseUrl.trim() === '';

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const stored = await aiProviderSave({
        id: provider?.id ?? null,
        name: name.trim(),
        kind: kind.id,
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        timeoutMs: provider?.timeoutMs ?? 0,
      });
      if (key.trim() !== '') await aiApiKeySet(stored.id, key.trim());
      onSaved();
    } catch (caught) {
      const reason =
        caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('saving failed', '没存上');
      setError(tr(`Nothing was changed — ${reason}.`, `什么都没改——${reason}。`));
    } finally {
      setBusy(false);
    }
  };

  const clearKey = () => {
    if (provider === null) return;
    aiApiKeyClear(provider.id)
      .catch(() => undefined)
      .finally(onSaved);
  };

  return (
    <>
      <SettingRow label={tr('Provider', '服务方')}>
        <Choice
          label={tr('Provider kind', '服务方类型')}
          options={PROVIDER_KINDS.map((option) => ({
            value: option.id,
            label: tr(option.label, option.labelZh),
          }))}
          value={kind.id}
          onChange={setKindId}
        />
      </SettingRow>
      <SettingRow label={tr('Details', '怎么连')} narrow>
        <div className="tvs-form">
          <LineField
            label={tr('Name', '名称')}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <LineField
            label={tr('Model', '模型')}
            mono
            value={model}
            onChange={(event) => setModel(event.target.value)}
          />
          <LineField
            label={tr('Address', '地址')}
            mono
            value={baseUrl}
            placeholder={
              kind.hasDefault ? tr('the default for this kind', '这一类的默认地址') : 'https://…/v1'
            }
            onChange={(event) => setBaseUrl(event.target.value)}
          />
          {(kind.needsKey || provider !== null) && (
            <LineField
              label="API Key"
              mono
              type="password"
              autoComplete="off"
              value={key}
              placeholder={
                provider?.hasApiKey === true
                  ? tr('kept — type to replace', '已存 —— 输入即替换')
                  : ''
              }
              onChange={(event) => setKey(event.target.value)}
            />
          )}
        </div>
        {error !== null && <Said>{error}</Said>}
        <Actions>
          <TextAction
            primary
            disabled={busy || name.trim() === '' || model.trim() === '' || missingKey || missingUrl}
            onClick={() => void save()}
          >
            {provider === null ? tr('Connect', '接上') : tr('Save', '保存')}
          </TextAction>
          <TextAction onClick={onCancel}>{tr('Cancel', '取消')}</TextAction>
          {provider?.hasApiKey === true && (
            <DangerAction onClick={clearKey}>{tr('Remove the key', '移除 Key')}</DangerAction>
          )}
        </Actions>
      </SettingRow>
    </>
  );
}

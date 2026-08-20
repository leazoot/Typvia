/**
 * AI — the section the design rebuilt hardest: a current-provider
 * card (the page's only card), a quiet provider list with hover actions, an
 * inline sheet for add/edit, the visual privacy promise, and the outbound
 * activity micro-ledger. API keys are write-only: replace, never reveal.
 */
import {
  aiApiKeyClear,
  aiApiKeySet,
  aiCheckConnectivity,
  aiEgressLogList,
  aiProviderDelete,
  aiProviderList,
  aiProviderSave,
  ipcErrorCopy,
  type AiEgressEntry,
  type AiProvider,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { OLLAMA_KIND, PROVIDER_KINDS, kindLabel, type KindOption } from '@typvia/ui/ai';
import { useEffect, useState } from 'react';
import { PrefSection } from './pref-section';
import {
  Chip,
  Dot,
  Fold,
  FoldRow,
  KV,
  OverflowMenu,
  SetLabel,
  SetRow,
  StatusWord,
} from './pref-kit';

function egressToday(entries: AiEgressEntry[]): AiEgressEntry[] {
  const dayStart = new Date();
  dayStart.setHours(0, 0, 0, 0);
  return entries.filter((entry) => entry.occurredAt >= dayStart.getTime());
}

export function AiPref() {
  const tr = useTr();
  const [providers, setProviders] = useState<AiProvider[] | null>(null);
  const [egress, setEgress] = useState<{ entries: AiEgressEntry[]; total: number } | null>(null);
  const [sheet, setSheet] = useState<'closed' | 'add' | string>('closed');
  const [probe, setProbe] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [activityOpen, setActivityOpen] = useState(false);

  const refresh = () =>
    aiProviderList()
      .then(setProviders)
      .catch(() => setProviders([]));

  useEffect(() => {
    void refresh();
    aiEgressLogList(30, 0)
      .then(setEgress)
      .catch(() => undefined);
  }, []);

  const current = providers?.[0] ?? null;

  const test = async (provider: AiProvider) => {
    setProbe(tr('Testing…', '测试中…'));
    try {
      const result = await aiCheckConnectivity(provider.id);
      setProbe(
        result.modelAvailable === false
          ? tr(
              `Reachable — model “${provider.model}” not listed`,
              `可连接——未发现模型「${provider.model}」`,
            )
          : tr('Connected', '连接正常'),
      );
    } catch (caught) {
      setProbe(
        caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('Unreachable', '无法连接'),
      );
    }
  };

  const remove = async (provider: AiProvider) => {
    await aiProviderDelete(provider.id).catch(() => undefined);
    await refresh();
  };

  const today = egress ? egressToday(egress.entries) : [];

  return (
    <PrefSection
      id="ai"
      glyph="ai"
      name="AI"
      sub={
        providers === null
          ? '…'
          : current
            ? `${current.name} · ${current.model}`
            : tr('Optional — everything works without it', '可选——不用它一切照常')
      }
      status={current ? <StatusWord kind="ok">{tr('Ready', '就绪')}</StatusWord> : undefined}
    >
      {current && (
        <>
          <SetLabel>{tr('Current provider', '当前 Provider')}</SetLabel>
          <div className="tvp-card">
            <div className="tvp-card-name">{current.name}</div>
            <div className="tvp-card-model">{current.model}</div>
            <div className="tvp-card-meta">
              <span>{kindLabel(current.kind, tr)}</span>
              <span className="tvp-status-inline">
                <Dot kind="ok" />
                {probe ?? tr('Ready', '就绪')}
              </span>
            </div>
            {current.hasApiKey && (
              <span className="tvp-keychain">
                <svg
                  width="11"
                  height="11"
                  viewBox="0 0 16 16"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  aria-hidden="true"
                >
                  <circle cx="8" cy="5.5" r="3" />
                  <path d="M8 8.5V14M8 11.5h2.6" />
                </svg>
                {tr('API key stored in the macOS Keychain', 'API Key 存于 macOS 钥匙串')}
              </span>
            )}
          </div>
        </>
      )}

      <SetLabel>Providers</SetLabel>
      {providers !== null && providers.length === 0 && (
        <SetRow
          title={tr('No provider yet', '尚未配置 Provider')}
          description={tr(
            'Connect a local model or a compatible endpoint to get suggestions.',
            '连接本地模型或兼容端点后即可获得建议。',
          )}
        />
      )}
      {providers?.map((provider) => (
        <SetRow
          key={provider.id}
          title={provider.name}
          description={`${kindLabel(provider.kind, tr)} · ${provider.model}`}
          status={<Dot kind="ok" />}
          actions={
            <>
              <Chip onClick={() => void test(provider)}>{tr('Test', '测试')}</Chip>
              <Chip onClick={() => setSheet(provider.id)}>{tr('Edit', '编辑')}</Chip>
              <OverflowMenu
                label={tr('Provider actions', 'Provider 操作')}
                items={[
                  'divider',
                  {
                    label: tr('Delete', '删除'),
                    danger: true,
                    onSelect: () => void remove(provider),
                  },
                ]}
              />
            </>
          }
        />
      ))}
      <SetRow
        onClick={() => setSheet(sheet === 'closed' ? 'add' : 'closed')}
        title={
          <span style={{ color: 'var(--tvp-secondary)' }}>
            {tr('＋ Add provider', '＋ 添加 Provider')}
          </span>
        }
      />
      <ProviderSheet
        key={sheet}
        mode={sheet}
        provider={providers?.find((candidate) => candidate.id === sheet) ?? null}
        onClose={() => setSheet('closed')}
        onSaved={() => {
          setSheet('closed');
          void refresh();
        }}
      />

      <SetLabel>{tr('Privacy', '隐私')}</SetLabel>
      <div className="tvp-promise">
        <div className="tvp-promise-t">
          <Dot kind="ok" />
          {tr('Local by default', '默认留在本机')}
        </div>
        <div className="tvp-promise-d">
          {tr(
            'Content is sent only when you ask for AI. Sensitive snippets never leave this Mac.',
            '只有你主动使用 AI 时,内容才会发送给 Provider。敏感片段永远不会离开这台 Mac。',
          )}
        </div>
      </div>

      <SetLabel>{tr('Outbound activity · today', '出站活动 · 今天')}</SetLabel>
      <div className="tvp-ledger">
        <div className="tvp-ledger-row">
          <span className="tvp-ledger-name">AI</span>
          {today.length === 0 ? (
            <span className="tvp-ledger-none">—</span>
          ) : (
            <span className="tvp-marks">
              {today.slice(0, 24).map((entry) => (
                <span key={entry.id} className="tvp-mark">
                  <span className="tvp-tip">
                    {new Date(entry.occurredAt).toLocaleTimeString([], {
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                    {' · '}
                    {entry.requestClass}
                    <br />
                    {tr(
                      `${String(entry.requestBytes)} bytes`,
                      `${String(entry.requestBytes)} 字节`,
                    )}
                  </span>
                </span>
              ))}
            </span>
          )}
          <span className="tvp-ledger-count">
            {tr(
              `${String(today.length)} request${today.length === 1 ? '' : 's'}`,
              `${String(today.length)} 次`,
            )}
          </span>
        </div>
      </div>
      <FoldRow
        label={tr(
          `All time · ${String(egress?.total ?? 0)}`,
          `累计 · ${String(egress?.total ?? 0)} 次`,
        )}
        open={activityOpen}
        onToggle={() => setActivityOpen((open) => !open)}
      />
      <Fold open={activityOpen}>
        {egress && egress.entries.length > 0 ? (
          <KV
            rows={egress.entries
              .slice(0, 10)
              .map((entry) => [
                new Date(entry.occurredAt).toLocaleString(),
                `${entry.requestClass} · ${String(entry.requestBytes)} ${tr('bytes', '字节')}`,
              ])}
          />
        ) : (
          <span className="tvp-note">
            {tr('Nothing has left this Mac.', '这台 Mac 没有发出过任何内容。')}
          </span>
        )}
      </Fold>

      <FoldRow
        label={tr('Advanced', '高级')}
        open={advancedOpen}
        onToggle={() => setAdvancedOpen((open) => !open)}
      />
      <Fold open={advancedOpen}>
        <KV
          rows={[
            [
              tr('Endpoint', '端点'),
              current?.baseUrl === ''
                ? tr('kind default', '按类型默认')
                : (current?.baseUrl ?? '—'),
            ],
            [tr('Timeout', '超时'), current ? `${String(current.timeoutMs / 1000)}s` : '—'],
            [
              tr('Egress log', '出站日志'),
              tr('always on — it cannot be disabled', '始终记录——不可关闭'),
            ],
          ]}
        />
      </Fold>
    </PrefSection>
  );
}

/** Inline sheet for adding or editing a provider. */
function ProviderSheet({
  mode,
  provider,
  onClose,
  onSaved,
}: {
  mode: 'closed' | 'add' | string;
  provider: AiProvider | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const tr = useTr();
  const editing = provider !== null;
  const [kind, setKind] = useState<KindOption>(
    editing
      ? (PROVIDER_KINDS.find((option) => option.id === provider.kind) ?? OLLAMA_KIND)
      : OLLAMA_KIND,
  );
  const [name, setName] = useState(provider?.name ?? '');
  const [model, setModel] = useState(provider?.model ?? '');
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? '');
  const [key, setKey] = useState('');
  const [advanced, setAdvanced] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const open = mode !== 'closed';
  const missingKey = kind.needsKey && !editing && key.trim() === '';
  const missingUrl = !kind.hasDefault && baseUrl.trim() === '';

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const saved = await aiProviderSave({
        id: provider?.id ?? null,
        name: name.trim(),
        kind: kind.id,
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        timeoutMs: provider?.timeoutMs ?? 0,
      });
      if (key.trim() !== '') await aiApiKeySet(saved.id, key.trim());
      onSaved();
    } catch (caught) {
      setError(
        caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('Could not save', '无法保存'),
      );
    } finally {
      setBusy(false);
    }
  };

  const clearKey = async () => {
    if (!provider) return;
    await aiApiKeyClear(provider.id).catch(() => undefined);
    onSaved();
  };

  return (
    <div className={`tvp-sheet-fold ${open ? 'open' : ''}`}>
      <div className="tvp-sheet-clip">
        {open && (
          <div className="tvp-sheet">
            <h3>
              {editing
                ? tr('Edit provider', '编辑 Provider')
                : tr('Add AI provider', '添加 AI Provider')}
            </h3>
            <p className="tvp-sheet-q">{tr('How do you want to connect?', '想怎么连接?')}</p>
            <div className="tvp-kinds" role="group" aria-label={tr('Provider kind', '服务方类型')}>
              {PROVIDER_KINDS.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  className="tvp-kind"
                  aria-pressed={option.id === kind.id}
                  onClick={() => setKind(option)}
                >
                  {tr(option.label, option.labelZh)}
                </button>
              ))}
            </div>
            <p className="tvp-note">{tr(kind.hint, kind.hintZh)}</p>
            <div className="tvp-fields">
              <div className="tvp-field">
                <label htmlFor="tvp-p-name">{tr('Name', '名称')}</label>
                <input
                  id="tvp-p-name"
                  value={name}
                  spellCheck={false}
                  onChange={(event) => setName(event.target.value)}
                />
              </div>
              <div className="tvp-field">
                <label htmlFor="tvp-p-model">{tr('Model', '模型')}</label>
                <input
                  id="tvp-p-model"
                  value={model}
                  spellCheck={false}
                  onChange={(event) => setModel(event.target.value)}
                />
              </div>
              {(kind.needsKey || editing) && (
                <div className="tvp-field">
                  <label htmlFor="tvp-p-key">API Key</label>
                  <input
                    id="tvp-p-key"
                    type="password"
                    value={key}
                    placeholder={provider?.hasApiKey ? '••••••••••••••••' : ''}
                    autoComplete="off"
                    onChange={(event) => setKey(event.target.value)}
                  />
                  <span className="tvp-keychain">
                    <svg
                      width="11"
                      height="11"
                      viewBox="0 0 16 16"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      aria-hidden="true"
                    >
                      <circle cx="8" cy="5.5" r="3" />
                      <path d="M8 8.5V14M8 11.5h2.6" />
                    </svg>
                    {tr(
                      'Stored in the macOS Keychain — replace, never revealed',
                      '存于 macOS 钥匙串——只可替换,不可回读',
                    )}
                    {provider?.hasApiKey && (
                      <Chip onClick={() => void clearKey()}>{tr('Remove key', '移除 Key')}</Chip>
                    )}
                  </span>
                </div>
              )}
            </div>
            <FoldRow
              label={tr('Advanced', '高级')}
              open={advanced}
              onToggle={() => setAdvanced((current) => !current)}
            />
            <Fold open={advanced}>
              <div className="tvp-fields" style={{ marginTop: 0 }}>
                <div className="tvp-field">
                  <label htmlFor="tvp-p-url">Base URL</label>
                  <input
                    id="tvp-p-url"
                    value={baseUrl}
                    spellCheck={false}
                    placeholder={
                      kind.hasDefault ? tr('kind default', '按类型默认') : 'https://…/v1'
                    }
                    onChange={(event) => setBaseUrl(event.target.value)}
                  />
                </div>
              </div>
            </Fold>
            {error && <p className="tvp-error">{error}</p>}
            <div className="tvp-sheet-foot">
              <button type="button" className="tvp-btn quiet" onClick={onClose}>
                {tr('Cancel', '取消')}
              </button>
              <button
                type="button"
                className="tvp-btn"
                disabled={
                  busy || name.trim() === '' || model.trim() === '' || missingKey || missingUrl
                }
                onClick={() => void save()}
              >
                {editing ? tr('Save', '保存') : tr('Add', '添加')}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

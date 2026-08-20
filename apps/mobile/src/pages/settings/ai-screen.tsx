/**
 * Settings · AI. The pushed screen has no dedicated design, so it is built
 * in the shared settings language. Configures the providers that power
 * organizing and actions on this phone — providers and keys are per-device
 * and never sync. Keys go straight to the platform secure store and are
 * never shown back; the row only reports that one is stored.
 */
import {
  aiApiKeyClear,
  aiApiKeySet,
  type AiProvider,
  aiProviderDelete,
  aiProviderList,
  aiProviderSave,
  ipcErrorCopy,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { OLLAMA_KIND, PROVIDER_KINDS, kindLabel, type KindOption } from '@typvia/ui/ai';
import { useEffect, useState } from 'react';
import { SettingsGroup, SettingsRow } from './rows';
import { ScreenSkeleton, SettingsScreen } from './screen';

interface AiScreenProps {
  onBack: () => void;
}

/** The provider form; `id === null` adds a new provider. */
interface ProviderForm {
  id: string | null;
  name: string;
  kind: KindOption;
  baseUrl: string;
  model: string;
  key: string;
  hasApiKey: boolean;
}

function emptyForm(): ProviderForm {
  return {
    id: null,
    name: '',
    kind: OLLAMA_KIND,
    baseUrl: '',
    model: '',
    key: '',
    hasApiKey: false,
  };
}

function toForm(provider: AiProvider): ProviderForm {
  return {
    id: provider.id,
    name: provider.name,
    kind: PROVIDER_KINDS.find((option) => option.id === provider.kind) ?? OLLAMA_KIND,
    baseUrl: provider.baseUrl,
    model: provider.model,
    key: '',
    hasApiKey: provider.hasApiKey,
  };
}

export function MobileAiScreen({ onBack }: AiScreenProps) {
  const tr = useTr();
  // `null` while the first load is in flight; a failure reads as a note.
  const [providers, setProviders] = useState<AiProvider[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [form, setForm] = useState<ProviderForm | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const reload = async () => {
    try {
      setProviders(await aiProviderList());
    } catch {
      setLoadFailed(true);
    }
  };

  useEffect(() => {
    void reload();
    // Initial load only; later reloads are explicit (save / delete).
  }, []);

  const save = async () => {
    if (form === null || busy) return;
    setBusy(true);
    setNote(null);
    try {
      const saved = await aiProviderSave({
        id: form.id,
        name: form.name.trim() === '' ? form.kind.label : form.name.trim(),
        kind: form.kind.id,
        baseUrl: form.baseUrl.trim(),
        model: form.model.trim(),
        timeoutMs: 0,
      });
      if (form.key.trim() !== '') {
        await aiApiKeySet(saved.id, form.key.trim());
      }
      setForm(null);
      await reload();
    } catch (caught) {
      setNote(
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the save failed — try again', '保存失败 —— 请重试'),
      );
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (form === null || form.id === null || busy) return;
    setBusy(true);
    setNote(null);
    try {
      await aiProviderDelete(form.id);
      setForm(null);
      setConfirmingDelete(false);
      await reload();
    } catch {
      setNote(tr('the delete failed — try again', '删除失败 —— 请重试'));
    } finally {
      setBusy(false);
    }
  };

  const clearKey = async () => {
    if (form === null || form.id === null || busy) return;
    setBusy(true);
    setNote(null);
    try {
      await aiApiKeyClear(form.id);
      setForm({ ...form, hasApiKey: false, key: '' });
      await reload();
    } catch {
      setNote(tr('the key could not be removed — try again', '密钥暂时移除不了 —— 请重试'));
    } finally {
      setBusy(false);
    }
  };

  if (providers === null && !loadFailed) {
    return <ScreenSkeleton parent={tr('Settings', '设置')} onBack={onBack} />;
  }

  return (
    <SettingsScreen
      parent={tr('Settings', '设置')}
      title="AI"
      intro={tr(
        'Everything works without AI. A provider set up here powers organizing and actions in this app only — the keyboard never calls AI.',
        '没有 AI 一切也照常工作。在这里设置的提供方只驱动本应用内的整理与操作 —— 键盘从不调用 AI。',
      )}
      onBack={onBack}
    >
      {loadFailed ? (
        <p className="tv-mset-note">
          {tr(
            'Your settings are safe — the provider list just could not load. Leave and reopen to try again.',
            '你的设置安好 —— 只是提供方列表暂时加载不出来。退出后重新打开再试。',
          )}
        </p>
      ) : (
        <>
          <SettingsGroup label={tr('Providers', '提供方')}>
            {providers !== null && providers.length === 0 && (
              <SettingsRow
                label={tr('No provider yet', '还没有提供方')}
                sub={tr('Add one below to turn AI on.', '在下方添加一个即可开启 AI。')}
              />
            )}
            {providers?.map((provider) => (
              <SettingsRow
                key={provider.id}
                label={provider.name}
                sub={kindLabel(provider.kind, tr)}
                value={provider.model}
                onPress={() => {
                  setForm(toForm(provider));
                  setConfirmingDelete(false);
                  setNote(null);
                }}
              />
            ))}
          </SettingsGroup>

          {form === null ? (
            <button
              type="button"
              className="tv-mset-button"
              onClick={() => {
                setForm(emptyForm());
                setConfirmingDelete(false);
                setNote(null);
              }}
            >
              {tr('Add provider', '添加提供方')}
            </button>
          ) : (
            <div className="tv-mset-ai-form">
              <div
                className="tv-mset-kinds"
                role="group"
                aria-label={tr('Provider kind', '提供方类型')}
              >
                {PROVIDER_KINDS.map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    className="tv-mset-button"
                    aria-pressed={option.id === form.kind.id}
                    onClick={() => setForm({ ...form, kind: option })}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
              <p className="tv-mset-note">{form.kind.hint}</p>

              <label className="tv-mset-field-label" htmlFor="tv-mset-ai-name">
                {tr('Name', '名称')}
              </label>
              <input
                id="tv-mset-ai-name"
                className="tv-mset-field"
                type="text"
                placeholder={form.kind.label}
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
              />

              <label className="tv-mset-field-label" htmlFor="tv-mset-ai-url">
                Base URL
              </label>
              <input
                id="tv-mset-ai-url"
                className="tv-mset-field"
                type="url"
                autoCapitalize="none"
                autoCorrect="off"
                placeholder={
                  form.kind.hasDefault
                    ? tr('Blank = local default', '留空 = 本地默认地址')
                    : 'https://…/v1'
                }
                value={form.baseUrl}
                onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
              />

              <label className="tv-mset-field-label" htmlFor="tv-mset-ai-model">
                {tr('Model', '模型')}
              </label>
              <input
                id="tv-mset-ai-model"
                className="tv-mset-field"
                type="text"
                autoCapitalize="none"
                autoCorrect="off"
                value={form.model}
                onChange={(e) => setForm({ ...form, model: e.target.value })}
              />

              <label className="tv-mset-field-label" htmlFor="tv-mset-ai-key">
                {tr('API key', 'API 密钥')}
              </label>
              <input
                id="tv-mset-ai-key"
                className="tv-mset-field"
                type="password"
                autoCapitalize="none"
                autoCorrect="off"
                placeholder={
                  form.hasApiKey
                    ? tr(
                        'A key is stored — enter a new one to replace it',
                        '已存有一个密钥 —— 输入新密钥即可替换',
                      )
                    : form.kind.needsKey
                      ? tr('Required for this kind', '此类型必填')
                      : tr('Optional', '可选')
                }
                value={form.key}
                onChange={(e) => setForm({ ...form, key: e.target.value })}
              />
              {form.hasApiKey && (
                <button
                  type="button"
                  className="tv-mset-action"
                  disabled={busy}
                  onClick={() => void clearKey()}
                >
                  {tr('Remove the stored key', '移除已存的密钥')}
                </button>
              )}

              <div className="tv-mset-ai-form-bar">
                <button
                  type="button"
                  className="tv-mset-button is-primary"
                  disabled={busy || form.model.trim() === ''}
                  onClick={() => void save()}
                >
                  {form.id === null
                    ? tr('Add provider', '添加提供方')
                    : tr('Save changes', '保存更改')}
                </button>
                <button
                  type="button"
                  className="tv-mset-button"
                  disabled={busy}
                  onClick={() => setForm(null)}
                >
                  {tr('Cancel', '取消')}
                </button>
              </div>

              {form.id !== null &&
                (confirmingDelete ? (
                  <div className="tv-mset-ai-form-bar">
                    <button
                      type="button"
                      className="tv-mset-action is-danger"
                      disabled={busy}
                      onClick={() => void remove()}
                    >
                      {tr(`Delete “${form.name}”`, `删除「${form.name}」`)}
                    </button>
                    <button
                      type="button"
                      className="tv-mset-button"
                      onClick={() => setConfirmingDelete(false)}
                    >
                      {tr('Keep it', '保留')}
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    className="tv-mset-action is-danger"
                    onClick={() => setConfirmingDelete(true)}
                  >
                    {tr('Delete…', '删除…')}
                  </button>
                ))}

              {note !== null && (
                <p className="tv-mset-note" role="status">
                  {note}
                </p>
              )}
            </div>
          )}

          <p className="tv-mset-note">
            {tr(
              'Keys are stored in this phone’s secure storage and never shown back or synced.',
              '密钥只存放在这部手机的安全存储中,不回显、不同步。',
            )}
          </p>
        </>
      )}
    </SettingsScreen>
  );
}

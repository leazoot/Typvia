/**
 * Mobile Settings root. The screen is section labels over hairline rows —
 * no group cards, no icons. The design's Input rows (keyboard state, trigger
 * prefix, insert-on-tap) have no host commands yet and are deliberately
 * absent rather than faked; the real rows are grouped under the design's
 * section names: Appearance (Theme, Language), Sync & security (Sync,
 * Privacy) and More (AI).
 *
 * Pushed screens are pages, not sheets, and navigation between them is a cut
 * (motion `neverAnimate`). The stack is local state plus the Android back
 * handler each screen registers.
 */
import { aiProviderList, type AiProvider } from '@typvia/shared';
import { type LocalePref, type ThemePref, type Tr, useTr, useUiPrefs } from '@typvia/ui';
import { useSyncSettings } from '@typvia/ui/sync';
import { useEffect, useState } from 'react';
import {
  MobileConflictPage,
  MobilePairPage,
  MobileRecoverPage,
  MobileRecoveryPage,
  MobileSyncPage,
} from '../sync';
import { MobileAiScreen } from './ai-screen';
import { MobileEgressLogScreen, MobilePrivacyScreen } from './privacy-screen';
import { SettingsGroup, SettingsRow } from './rows';
import './settings.css';

type Screen =
  'root' | 'sync' | 'pair' | 'conflicts' | 'recovery' | 'recover' | 'ai' | 'privacy' | 'egress';

export function SettingsPage() {
  const tr = useTr();
  const [screen, setScreen] = useState<Screen>('root');
  const settings = useSyncSettings();
  const { status } = settings;
  // `null` while loading; a failure reads as "off" rather than an error —
  // the row is a summary, the screen itself reports precisely.
  const [providers, setProviders] = useState<AiProvider[] | null>(null);

  useEffect(() => {
    if (screen !== 'root') return;
    let cancelled = false;
    aiProviderList()
      .then((loaded) => {
        if (!cancelled) setProviders(loaded);
      })
      .catch(() => {
        if (!cancelled) setProviders([]);
      });
    return () => {
      cancelled = true;
    };
  }, [screen]);

  const backToSync = () => {
    setScreen('sync');
    void settings.refresh();
  };

  if (screen === 'sync') {
    return (
      <MobileSyncPage
        onBack={() => {
          setScreen('root');
          void settings.refresh();
        }}
        onPair={() => {
          setScreen('pair');
        }}
        onConflicts={() => {
          setScreen('conflicts');
        }}
        onRecovery={() => {
          setScreen('recovery');
        }}
        onRecover={() => {
          setScreen('recover');
        }}
      />
    );
  }

  if (screen === 'pair' && status !== null) {
    return <MobilePairPage status={status} onBack={backToSync} onDone={backToSync} />;
  }

  if (screen === 'conflicts') return <MobileConflictPage onBack={backToSync} />;

  if (screen === 'recovery') return <MobileRecoveryPage onBack={backToSync} />;

  if (screen === 'recover') {
    return (
      <MobileRecoverPage
        onBack={backToSync}
        onExportRecovery={() => {
          setScreen('recovery');
        }}
      />
    );
  }

  if (screen === 'ai') {
    return (
      <MobileAiScreen
        onBack={() => {
          setScreen('root');
        }}
      />
    );
  }

  if (screen === 'privacy') {
    return (
      <MobilePrivacyScreen
        onBack={() => {
          setScreen('root');
        }}
        onEgressLog={() => {
          setScreen('egress');
        }}
      />
    );
  }

  if (screen === 'egress') {
    return (
      <MobileEgressLogScreen
        onBack={() => {
          setScreen('privacy');
        }}
      />
    );
  }

  return (
    <main className="tv-mset">
      <header className="tv-mset-head">
        <h1 className="tv-mset-head-title">{tr('Settings', '设置')}</h1>
      </header>

      <AppearanceGroup />

      <SettingsGroup label={tr('Sync & security', '同步与安全')} annotated>
        {status === null ? (
          <SettingsRow
            label={tr('Sync', '同步')}
            sub={tr('Reading the sync state…', '正在读取同步状态…')}
          />
        ) : (
          <SettingsRow
            label={tr('Sync', '同步')}
            value={syncValue(status.configured, status.enabled, status.available, tr)}
            onPress={() => {
              setScreen('sync');
            }}
          />
        )}
        <SettingsRow
          label={tr('Privacy', '隐私')}
          onPress={() => {
            setScreen('privacy');
          }}
        />
      </SettingsGroup>
      <p className="tv-mset-note">
        {tr(
          'Sync copies your snippets between your devices as ciphertext. Turning it off leaves every feature working — nothing here is required.',
          '同步会把你的片段以密文形式在你的设备之间复制。关闭它后所有功能照常工作 —— 这里没有任何必需项。',
        )}
      </p>

      <SettingsGroup label={tr('More', '更多')}>
        <SettingsRow
          label="AI"
          value={aiValue(providers, tr)}
          onPress={() => {
            setScreen('ai');
          }}
        />
      </SettingsGroup>
    </main>
  );
}

/**
 * Theme and language under Appearance. Both are UI preferences kept
 * in localStorage; they apply immediately and follow the OS while set to
 * System. The chosen word carries a 2px accent underline — never a fill.
 */
function AppearanceGroup() {
  const tr = useTr();
  const { localePref, themePref, setLocalePref, setThemePref } = useUiPrefs();

  const themes: ReadonlyArray<{ id: ThemePref; label: string }> = [
    { id: 'light', label: tr('Light', '浅色') },
    { id: 'dark', label: tr('Dark', '深色') },
    { id: 'system', label: tr('System', '跟随系统') },
  ];
  const languages: ReadonlyArray<{ id: LocalePref; label: string }> = [
    { id: 'en', label: 'English' },
    { id: 'zh', label: '中文' },
    { id: 'system', label: tr('System', '跟随系统') },
  ];

  return (
    <SettingsGroup label={tr('Appearance', '外观')}>
      <SettingsRow
        label={tr('Theme', '主题')}
        control={
          <span className="tv-mset-choice" role="group" aria-label={tr('Theme', '主题')}>
            {themes.map((option) => (
              <button
                key={option.id}
                type="button"
                className="tv-mset-action"
                aria-pressed={option.id === themePref}
                onClick={() => setThemePref(option.id)}
              >
                {option.label}
              </button>
            ))}
          </span>
        }
      />
      <SettingsRow
        label={tr('Language', '语言')}
        control={
          <span className="tv-mset-choice" role="group" aria-label={tr('Language', '语言')}>
            {languages.map((option) => (
              <button
                key={option.id}
                type="button"
                className="tv-mset-action"
                aria-pressed={option.id === localePref}
                onClick={() => setLocalePref(option.id)}
              >
                {option.label}
              </button>
            ))}
          </span>
        }
      />
    </SettingsGroup>
  );
}

/**
 * The Sync row's right-hand reading. The design shows a device count; before
 * an account exists there is nothing to count, so the row says what is true
 * instead of showing "0 devices".
 */
function syncValue(configured: boolean, enabled: boolean, available: boolean, tr: Tr): string {
  if (!available) return tr('unavailable', '不可用');
  if (!configured) return tr('not set up', '未设置');
  return enabled ? tr('on', '已开启') : tr('off', '已关闭');
}

/** The AI row's right-hand reading: the one provider by name, a count when
 *  there are several, "off" when none — never a fake model name. */
function aiValue(providers: AiProvider[] | null, tr: Tr): string {
  if (providers === null) return '';
  if (providers.length === 0) return tr('off', '未启用');
  if (providers.length === 1) return providers[0]?.name ?? tr('on', '已启用');
  return tr(`${providers.length} providers`, `${providers.length} 个提供方`);
}

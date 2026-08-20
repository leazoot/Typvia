import { listen } from '@tauri-apps/api/event';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useEnterAdvances } from './onboarding-page';

interface ShortcutStepProps {
  onContinue: () => void;
}

/**
 * Step 4 — the shortcut, learned by pressing it. The host broadcasts a
 * payload-free `panel:summoned` when ⌘⇧V opens the panel; only that genuine
 * event enables the primary button. "Nothing happened?" opens the
 * blocked-by-the-OS guidance: what still works, then what to do.
 */
export function ShortcutStep({ onContinue }: ShortcutStepProps) {
  const tr = useTr();
  const [opened, setOpened] = useState(false);
  const [showHelp, setShowHelp] = useState(false);

  useEffect(() => {
    const pending = listen('panel:summoned', () => {
      setOpened(true);
    });
    return () => {
      void pending.then((unlisten) => {
        unlisten();
      });
    };
  }, []);

  useEnterAdvances(onContinue, opened);

  return (
    <>
      <section className="tv-onb-body">
        <h1 className="tv-onb-title">{tr('Press it now.', '现在按一次快捷键。')}</h1>
        <div className="tv-onb-shortcut">
          <div className="tv-onb-shortcut-keys" aria-label="Command Shift V">
            <kbd className="tv-onb-keycap">⌘</kbd>
            <kbd className="tv-onb-keycap">⇧</kbd>
            <kbd className="tv-onb-keycap is-accent">V</kbd>
          </div>
          <p className="tv-onb-shortcut-note">
            {tr(
              'Works in every app, over every window. The panel opens on top of whatever you are doing.',
              '在任何应用、任何窗口中都有效。面板会打开在你正在做的事情上方。',
            )}
          </p>
          <p className="tv-onb-shortcut-state" role="status">
            {opened
              ? tr(
                  'The panel opened — that is the whole gesture.',
                  '面板已打开——整个动作就是这样。',
                )
              : tr('Waiting for the press…', '等待按键中…')}
          </p>
        </div>
        {showHelp && (
          <div className="tv-onb-help">
            <p>
              {tr(
                'Opening the panel needs no permission — if nothing appeared, another app may own ⌘⇧V.',
                '打开面板不需要任何权限——如果没有任何反应，可能是其他应用占用了 ⌘⇧V。',
              )}
            </p>
            <p>
              {tr(
                'Inserting into other apps — and expanding abbreviations — needs Accessibility: System Settings → Privacy & Security → Accessibility → allow Typvia. Without it, Typvia still works — inserts land on your clipboard to paste yourself.',
                '插入到其他应用、以及展开缩写，需要辅助功能权限：系统设置 → 隐私与安全性 → 辅助功能 → 允许 Typvia。没有该权限 Typvia 也能使用——插入内容会放到剪贴板，由你自行粘贴。',
              )}
            </p>
          </div>
        )}
      </section>
      <footer className="tv-onb-foot">
        <button type="button" className="tv-onb-primary" disabled={!opened} onClick={onContinue}>
          {tr('It opened — continue', '已打开——继续')}
          <kbd>↵</kbd>
        </button>
        {!showHelp && (
          <button type="button" className="tv-onb-skip" onClick={() => setShowHelp(true)}>
            {tr('Nothing happened?', '没有反应？')}
          </button>
        )}
        {showHelp && (
          <button type="button" className="tv-onb-skip" onClick={onContinue}>
            {tr('Continue anyway', '仍然继续')}
          </button>
        )}
      </footer>
    </>
  );
}

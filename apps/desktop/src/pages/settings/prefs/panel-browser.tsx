/**
 * Browser extension — a design-uncovered surface folded into the same quiet
 * language: summary row + one switch row, honest about the helper's install
 * state.
 */
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { useBrowserIntegration } from '../../../browser-integration/browser-integration-context';
import { PrefSection } from './pref-section';
import { Chip, SetRow, StatusWord } from './pref-kit';

export function BrowserPref() {
  const tr = useTr();
  const { status, enable, disable } = useBrowserIntegration();
  const [busy, setBusy] = useState(false);

  const enabled = status?.enabled === true;
  const hostInstalled = status?.hostInstalled === true;

  const toggle = async () => {
    setBusy(true);
    try {
      await (enabled ? disable() : enable());
    } finally {
      setBusy(false);
    }
  };

  return (
    <PrefSection
      id="browser"
      glyph="browser"
      name={tr('Browser extension', '浏览器扩展')}
      sub={
        enabled
          ? hostInstalled
            ? tr('Snippets available inside the browser', '片段在浏览器内可用')
            : tr('On — the extension isn’t connected yet', '已开启——扩展尚未连接')
          : tr('Bring snippets into the browser', '把片段带进浏览器')
      }
      status={
        enabled ? (
          <StatusWord kind={hostInstalled ? 'ok' : 'warn'}>
            {hostInstalled ? tr('Connected', '已连接') : tr('Waiting', '等待连接')}
          </StatusWord>
        ) : undefined
      }
    >
      <SetRow
        title={
          enabled
            ? tr('Extension enabled', '扩展已启用')
            : tr('Use snippets in the browser', '在浏览器里使用片段')
        }
        description={
          enabled
            ? tr(
                'Sensitive snippets never reach the browser; turn off to freeze the snapshot.',
                '敏感片段永远不会进入浏览器;关闭即冻结快照。',
              )
            : tr(
                'Registers a local helper for the Typvia extension. Nothing leaves this Mac.',
                '为 Typvia 扩展注册本地宿主。任何内容都不会离开这台 Mac。',
              )
        }
        actions={
          <Chip cta disabled={busy || status === null} onClick={() => void toggle()}>
            {enabled ? tr('Turn off', '关闭') : tr('Turn on', '开启')}
          </Chip>
        }
      />
    </PrefSection>
  );
}

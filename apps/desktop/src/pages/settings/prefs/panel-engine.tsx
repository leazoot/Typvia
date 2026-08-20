/**
 * Expansion engine — the managed espanso engine in the peek/fold
 * language: status word up front, coexistence prompts inline, the config
 * path abbreviated until clicked, turn-off buried at the bottom of the
 * overflow menu.
 */
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { useEspanso } from '../../../espanso/espanso-context';
import { PrefSection } from './pref-section';
import {
  Chip,
  Fold,
  FoldRow,
  KV,
  OverflowMenu,
  SetLabel,
  SetRow,
  StatusWord,
  type DotKind,
} from './pref-kit';

export function EnginePref() {
  const tr = useTr();
  const { status, enable, disable, coexist, refresh } = useEspanso();
  const [busy, setBusy] = useState(false);
  const [pathOpen, setPathOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const state = status?.state ?? 'unavailable';
  const triggerCount = status?.triggerCount ?? 0;

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };

  const copyPath = async () => {
    if (!status?.configPath) return;
    try {
      await navigator.clipboard.writeText(status.configPath);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // Best-effort: the full path is revealed on click regardless.
    }
  };

  const statusWord: { kind: DotKind; word: string } =
    state === 'running'
      ? { kind: 'ok', word: tr('Running', '运行中') }
      : state === 'retrying'
        ? { kind: 'warn', word: tr('Restarting', '重启中') }
        : state === 'conflict' || state === 'takeover_pending'
          ? { kind: 'warn', word: tr('Needs a decision', '待选择') }
          : state === 'standing_aside'
            ? { kind: 'idle', word: tr('Standing aside', '已退避') }
            : state === 'off'
              ? { kind: 'idle', word: tr('Off', '未开启') }
              : {
                  kind: 'warn',
                  word: state === 'failed' ? tr('Stopped', '已停止') : tr('Unavailable', '不可用'),
                };

  const sub =
    state === 'running'
      ? tr(
          `${String(triggerCount)} trigger${triggerCount === 1 ? '' : 's'} at work`,
          `${String(triggerCount)} 个触发词正在工作`,
        )
      : state === 'conflict' || state === 'takeover_pending'
        ? tr('Another expander is running', '检测到另一个展开工具在运行')
        : state === 'standing_aside'
          ? tr('Your own Espanso keeps running', '你的 Espanso 继续运行')
          : state === 'failed'
            ? tr('Your snippets are safe — expansion is paused', '你的片段完好——展开暂停')
            : state === 'off'
              ? tr('Turn on to expand as you type', '开启后输入时自动展开')
              : tr('Not included in this build', '当前构建未包含引擎');

  return (
    <PrefSection
      id="engine"
      glyph="engine"
      name={tr('Expansion engine', '展开引擎')}
      sub={sub}
      status={<StatusWord kind={statusWord.kind}>{statusWord.word}</StatusWord>}
      actions={
        state === 'failed' || state === 'off' ? (
          <Chip cta disabled={busy} onClick={() => void run(enable)}>
            {tr('Turn on', '开启')}
          </Chip>
        ) : state === 'running' ? (
          <Chip disabled={busy} onClick={() => void run(enable)}>
            {tr('Restart', '重启')}
          </Chip>
        ) : undefined
      }
    >
      <SetLabel>{tr('Status', '状态')}</SetLabel>

      {state === 'running' && (
        <SetRow
          title={tr('Engine running', '引擎运行中')}
          description={tr('Changes apply instantly.', '修改会立即生效。')}
          actions={
            <OverflowMenu
              label={tr('Engine actions', '引擎操作')}
              items={[
                { label: tr('Restart engine', '重启引擎'), onSelect: () => void run(enable) },
                {
                  label: copied ? tr('Copied', '已复制') : tr('Copy config path', '拷贝配置路径'),
                  onSelect: () => void copyPath(),
                },
                'divider',
                { label: tr('Turn off', '关闭'), danger: true, onSelect: () => void run(disable) },
              ]}
            />
          }
        />
      )}

      {(state === 'off' || state === 'failed' || state === 'unavailable') && (
        <SetRow
          title={
            state === 'unavailable'
              ? tr('Engine not included in this build', '当前构建未包含引擎')
              : state === 'failed'
                ? tr('Engine stopped', '引擎已停止')
                : tr('Engine off', '引擎未开启')
          }
          description={tr(
            'Snippets, search and copy all work — abbreviations just won’t expand on their own.',
            '片段、搜索与复制一切照常——只是缩写不会自动展开。',
          )}
          actions={
            state !== 'unavailable' ? (
              <Chip cta disabled={busy} onClick={() => void run(enable)}>
                {state === 'failed' ? tr('Start engine', '启动引擎') : tr('Turn on', '开启')}
              </Chip>
            ) : undefined
          }
        />
      )}

      {state === 'conflict' && (
        <SetRow
          title={tr('Another expander is running', '检测到另一个展开工具在运行')}
          description={tr(
            'Your own Espanso is active. Two engines would fight over every keystroke, so Typvia’s engine is holding off until you choose.',
            '你自行安装的 Espanso 正在运行。两个引擎会争抢同一段按键,Typvia 引擎已暂缓启动,等待你的选择。',
          )}
          actions={
            <>
              <Chip cta disabled={busy} onClick={() => void run(() => coexist('takeover'))}>
                {tr('Use Typvia’s engine', '改用 Typvia 引擎')}
              </Chip>
              <Chip disabled={busy} onClick={() => void run(() => coexist('stand_aside'))}>
                {tr('Keep my Espanso', '继续用我的 Espanso')}
              </Chip>
            </>
          }
        />
      )}

      {state === 'takeover_pending' && (
        <SetRow
          title={tr('Waiting for your Espanso to stop', '等待你的 Espanso 停止')}
          description={tr(
            'Quit it yourself — Typvia never shuts it down for you. The engine starts on its own once it’s gone.',
            '请自行退出它——Typvia 不会替你关闭。它停止后引擎会自动启动。',
          )}
          actions={
            <>
              <Chip disabled={busy} onClick={() => void run(refresh)}>
                {tr('Check again', '再次检查')}
              </Chip>
              <Chip disabled={busy} onClick={() => void run(() => coexist('stand_aside'))}>
                {tr('Keep my Espanso instead', '还是继续用我的 Espanso')}
              </Chip>
            </>
          }
        />
      )}

      {state === 'standing_aside' && (
        <SetRow
          title={tr('Standing aside', '已退避')}
          description={tr(
            'Your own Espanso keeps running and Typvia’s engine stays off — Typvia abbreviations don’t expand.',
            '你的 Espanso 继续运行,Typvia 引擎保持关闭——Typvia 的缩写不会展开。',
          )}
          actions={
            <Chip cta disabled={busy} onClick={() => void run(() => coexist('takeover'))}>
              {tr('Use Typvia’s engine instead', '改用 Typvia 引擎')}
            </Chip>
          }
        />
      )}

      {status?.configPath && (
        <>
          <SetLabel>{tr('Config', '配置')}</SetLabel>
          <SetRow
            onClick={() => setPathOpen((current) => !current)}
            title={
              <span className="tvp-mono">~/…/{status.configPath.split('/').slice(-1)[0]}</span>
            }
            actions={
              <Chip onClick={() => void copyPath()}>
                {copied ? tr('Copied', '已复制') : tr('Copy', '拷贝')}
              </Chip>
            }
          />
          <Fold open={pathOpen}>
            <span className="tvp-mono">{status.configPath}</span>
          </Fold>
        </>
      )}

      <FoldRow
        label={tr('Advanced', '高级')}
        open={advancedOpen}
        onToggle={() => setAdvancedOpen((current) => !current)}
      />
      <Fold open={advancedOpen}>
        <KV
          rows={[
            [tr('Engine version', '引擎版本'), status?.version ?? '—'],
            [
              tr('Sensitive snippets', '敏感片段'),
              tr('never written to the engine', '永不写入引擎配置'),
            ],
            [
              tr('Permissions', '权限'),
              tr(
                'expanding shares the Accessibility grant with inserting (System Settings → Typvia)',
                '展开与插入共用辅助功能授权(系统设置 → Typvia)',
              ),
            ],
            [
              tr('Open source', '开源'),
              tr(
                'the engine is the Espanso project (GPL-3.0); its licence ships with the app',
                '引擎基于开源项目 Espanso(GPL-3.0),许可证随应用分发',
              ),
            ],
          ]}
        />
      </Fold>
    </PrefSection>
  );
}

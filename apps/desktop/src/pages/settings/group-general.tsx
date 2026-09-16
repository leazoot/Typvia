// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { useState, type ReactNode } from 'react';
import { useBrowserIntegration } from '../../browser-integration/browser-integration-context';
import { useEspanso } from '../../espanso/espanso-context';
import { GroupTitle, TextAction } from '../../paper/kit';
import { Actions, DangerAction, SettingRow, State, type StateKind } from './settings-kit';

/** What keeps working in the background: the expansion engine and the browser helper. */
export function GeneralGroup() {
  const tr = useTr();
  return (
    <section className="tvs-group">
      <GroupTitle>{tr('General', '通用')}</GroupTitle>
      <EngineRow />
      <BrowserRow />
    </section>
  );
}

/** Runs one engine call at a time; a second click while one is out does nothing. */
function useBusy(): [boolean, (action: () => Promise<void>) => void] {
  const [busy, setBusy] = useState(false);
  const run = (action: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    action()
      .catch(() => undefined)
      .finally(() => setBusy(false));
  };
  return [busy, run];
}

/** The last two parts of a path are all anyone reads; the copy action keeps the whole. */
function shortPath(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part !== '');
  return parts.length <= 2 ? path : `…/${parts.slice(-2).join('/')}`;
}

interface EngineView {
  kind: StateKind;
  word: string;
  /** Only a state that asks the user to decide carries a sentence. */
  ask: string | null;
  actions: ReactNode;
}

function EngineRow() {
  const tr = useTr();
  const { status, enable, disable, coexist, refresh } = useEspanso();
  const [, run] = useBusy();
  const [copied, setCopied] = useState(false);
  const state = status?.state ?? 'unavailable';
  const triggers = status?.triggerCount ?? 0;

  const copyPath = () => {
    if (!status?.configPath) return;
    navigator.clipboard
      .writeText(status.configPath)
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1400);
      })
      .catch(() => undefined);
  };

  const view: EngineView =
    state === 'running'
      ? {
          kind: 'ok',
          word: tr(
            `Running · ${String(triggers)} trigger${triggers === 1 ? '' : 's'}`,
            `在运行 · ${String(triggers)} 个触发词`,
          ),
          ask: null,
          actions: (
            <>
              <TextAction onClick={() => run(enable)}>{tr('Restart', '重启')}</TextAction>
              <DangerAction onClick={() => run(disable)}>
                {tr('Turn the engine off', '关掉引擎')}
              </DangerAction>
            </>
          ),
        }
      : state === 'retrying'
        ? { kind: 'warn', word: tr('Restarting', '正在重启'), ask: null, actions: null }
        : state === 'conflict'
          ? {
              kind: 'warn',
              word: tr('Waiting on you', '等你选'),
              ask: tr(
                'Your own Espanso is running. Two engines would fight over every keystroke, so Typvia’s engine holds off until you choose.',
                '你自己装的 Espanso 正在运行。两个引擎会抢同一串按键,所以 Typvia 的引擎先不启动,等你选。',
              ),
              actions: (
                <>
                  <TextAction primary onClick={() => run(() => coexist('takeover'))}>
                    {tr('Use Typvia’s engine', '改用 Typvia 引擎')}
                  </TextAction>
                  <TextAction onClick={() => run(() => coexist('stand_aside'))}>
                    {tr('Keep my Espanso', '继续用我的 Espanso')}
                  </TextAction>
                </>
              ),
            }
          : state === 'takeover_pending'
            ? {
                kind: 'warn',
                word: tr('Waiting for Espanso to quit', '等 Espanso 退出'),
                ask: tr(
                  'Quit it yourself — Typvia never shuts it down for you. The engine starts on its own once it is gone.',
                  '请你自己退出它——Typvia 不会替你关。它一停,引擎就自己启动。',
                ),
                actions: (
                  <>
                    <TextAction onClick={() => run(refresh)}>
                      {tr('Check again', '再看一次')}
                    </TextAction>
                    <TextAction onClick={() => run(() => coexist('stand_aside'))}>
                      {tr('Keep my Espanso instead', '还是用我的 Espanso')}
                    </TextAction>
                  </>
                ),
              }
            : state === 'standing_aside'
              ? {
                  kind: 'idle',
                  word: tr('Standing aside', '已让开'),
                  ask: null,
                  actions: (
                    <TextAction primary onClick={() => run(() => coexist('takeover'))}>
                      {tr('Use Typvia’s engine instead', '改用 Typvia 引擎')}
                    </TextAction>
                  ),
                }
              : state === 'off' || state === 'failed'
                ? {
                    kind: state === 'failed' ? 'warn' : 'idle',
                    word: state === 'failed' ? tr('Stopped', '停下了') : tr('Off', '没开'),
                    ask: null,
                    actions: (
                      <TextAction primary onClick={() => run(enable)}>
                        {state === 'failed'
                          ? tr('Start it again', '重新启动')
                          : tr('Turn on', '开启')}
                      </TextAction>
                    ),
                  }
                : {
                    kind: 'idle',
                    word: tr('Not in this build', '这个版本没带'),
                    ask: null,
                    actions: null,
                  };

  return (
    <>
      <SettingRow label={tr('Expansion engine', '展开引擎')}>
        {view.ask === null ? (
          <div className="tvs-inline">
            <State kind={view.kind}>{view.word}</State>
            {view.actions}
          </div>
        ) : (
          <>
            <State kind={view.kind}>{view.word}</State>
            <p className="tvs-text">{view.ask}</p>
            {view.actions !== null && <Actions>{view.actions}</Actions>}
          </>
        )}
      </SettingRow>
      {status?.configPath && (
        <SettingRow label={tr('Config file', '配置文件')}>
          <div className="tvs-inline">
            <span className="tvs-text is-mono" title={status.configPath}>
              {shortPath(status.configPath)}
            </span>
            <TextAction onClick={copyPath}>
              {copied ? tr('Copied', '已拷贝') : tr('Copy the path', '拷贝路径')}
            </TextAction>
          </div>
        </SettingRow>
      )}
    </>
  );
}

function BrowserRow() {
  const tr = useTr();
  const { status, enable, disable } = useBrowserIntegration();
  const [, run] = useBusy();
  const enabled = status?.enabled === true;
  const connected = status?.hostInstalled === true;

  return (
    <SettingRow label={tr('Browser extension', '浏览器扩展')}>
      <div className="tvs-inline">
        {enabled ? (
          <State kind={connected ? 'ok' : 'warn'}>
            {connected ? tr('Connected', '已连上') : tr('Waiting for the extension', '等扩展连上')}
          </State>
        ) : (
          <State kind="idle">{tr('Off', '没开')}</State>
        )}
        {status !== null &&
          (enabled ? (
            <DangerAction onClick={() => run(disable)}>{tr('Turn off', '关掉')}</DangerAction>
          ) : (
            <TextAction onClick={() => run(enable)}>{tr('Turn on', '开启')}</TextAction>
          ))}
      </div>
    </SettingRow>
  );
}

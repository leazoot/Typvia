// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { onboardingComplete } from '@typvia/shared';
import { Caret, useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { DoneStep } from './done-step';
import { FirstSnippetStep } from './first-snippet-step';
import './onboarding.css';
import { ShortcutStep } from './shortcut-step';

const STEP_COUNT = 5;

interface OnboardingPageProps {
  /** Called after the completion marker is handled; the shell takes over. */
  onDone: () => void;
}

/**
 * First-run flow: five steps, one idea per
 * screen, a visible way out on every screen, transitions are cuts. The flow
 * ends in the app itself — the panel rehearsal in step 4 is the product
 * proving itself.
 */
export function OnboardingPage({ onDone }: OnboardingPageProps) {
  const tr = useTr();
  const navigate = useNavigate();
  const [step, setStep] = useState(1);
  const [finishError, setFinishError] = useState(false);

  const finish = (destination?: string) => {
    onboardingComplete()
      .then(() => {
        if (destination !== undefined) navigate(destination);
        onDone();
      })
      .catch(() => {
        setFinishError(true);
      });
  };

  const forceFinish = () => {
    // The marker write failed and the user chose to continue: nothing is
    // lost — onboarding simply offers itself again next launch.
    onDone();
  };

  return (
    <main className="tv-onb">
      {/* Onboarding renders without the TopNav drag strip; with the Overlay
          title bar this header is what the window is dragged by. */}
      <header className="tv-onb-head" data-tauri-drag-region="">
        <span className="tv-onb-step-count">
          {tr(`Step ${step} of ${STEP_COUNT}`, `第 ${step} 步，共 ${STEP_COUNT} 步`)}
        </span>
        {step === 1 && (
          <button type="button" className="tv-onb-escape" onClick={() => finish()}>
            {tr('Skip setup', '跳过设置')}
          </button>
        )}
        {step > 1 && step < STEP_COUNT && (
          <button
            type="button"
            className="tv-onb-escape"
            onClick={() => setStep((s) => Math.max(1, s - 1))}
          >
            {tr('Back', '返回')}
          </button>
        )}
      </header>

      {step === 1 && <WelcomeStep onContinue={() => setStep(2)} />}
      {step === 2 && <StorageStep onContinue={() => setStep(3)} />}
      {step === 3 && <FirstSnippetStep onContinue={() => setStep(4)} />}
      {step === 4 && <ShortcutStep onContinue={() => setStep(5)} />}
      {step === 5 && <DoneStep onFinish={finish} />}

      {finishError && (
        <div className="tv-onb-finish-error" role="alert">
          <span>
            {tr(
              'Your snippets are safe — setup could not record completion, so it may show again next launch.',
              '你的片段安全无恙——设置未能记录完成状态，下次启动时可能会再次出现。',
            )}
          </span>
          <button type="button" className="tv-onb-ghost" onClick={() => finish()}>
            {tr('Try again', '重试')}
          </button>
          <button type="button" className="tv-onb-ghost" onClick={forceFinish}>
            {tr('Continue anyway', '仍然继续')}
          </button>
        </div>
      )}
    </main>
  );
}

interface StepProps {
  onContinue: () => void;
}

/** Step 1 — a statement, not a pitch: the 58px tagline with the caret is the brand mark. */
function WelcomeStep({ onContinue }: StepProps) {
  const tr = useTr();
  useEnterAdvances(onContinue);
  return (
    <>
      <section className="tv-onb-body tv-onb-welcome">
        <div className="tv-onb-tagline-row">
          <Caret height={54} />
          <h1 className="tv-onb-tagline">
            {tr('Save once.', '保存一次，')}
            <br />
            {tr('Type anywhere.', '随处输入。')}
          </h1>
        </div>
        <p className="tv-onb-welcome-lead">
          {tr(
            'Typvia keeps the text you retype — commands, replies, prompts, keys — and puts it back wherever your cursor is. Everything stays on this Mac unless you ask otherwise.',
            'Typvia 保存你反复输入的文本——命令、回复、提示词、密钥——并把它放回光标所在的任何地方。除非你另有要求，一切都只留在这台 Mac 上。',
          )}
        </p>
        <div className="tv-onb-actions">
          <button type="button" className="tv-onb-primary" onClick={onContinue}>
            {tr('Set up in four steps', '用四步完成设置')}
            <kbd>↵</kbd>
          </button>
          <span className="tv-onb-aside">{tr('takes about a minute', '大约需要一分钟')}</span>
        </div>
      </section>
      <footer className="tv-onb-foot">
        <span>
          {tr(
            'Open source · local-first · end-to-end encrypted when synced',
            '开源 · 本地优先 · 同步时端到端加密',
          )}
        </span>
      </footer>
    </>
  );
}

/** Step 2 — where it lives: local-only first, sync described without persuasion. */
function StorageStep({ onContinue }: StepProps) {
  const tr = useTr();
  useEnterAdvances(onContinue);
  return (
    <>
      <section className="tv-onb-body">
        <h1 className="tv-onb-title">{tr('Where should this live?', '内容应该存放在哪里？')}</h1>
        <div className="tv-onb-cards">
          <div className="tv-onb-card is-selected">
            <span aria-hidden="true" className="tv-onb-card-glyph" />
            <h2 className="tv-onb-card-title">{tr('This Mac only', '仅这台 Mac')}</h2>
            <p className="tv-onb-card-body">
              {tr(
                'Nothing ever leaves this machine. No account, no server, no network calls. You can turn on sync any time later.',
                '任何内容都不会离开这台机器。无需账户、无服务器、无网络请求。之后随时可以开启同步。',
              )}
            </p>
            <p className="tv-onb-card-state">
              <kbd>↵</kbd> {tr('Selected', '已选择')}
            </p>
          </div>
          <div className="tv-onb-card is-muted">
            <span aria-hidden="true" className="tv-onb-card-glyph is-chain" />
            <h2 className="tv-onb-card-title">{tr('Sync my devices', '多设备同步')}</h2>
            <p className="tv-onb-card-body">
              {tr(
                'End-to-end encrypted. We can hold the encrypted blobs, or point it at your own server. Keys never leave your devices.',
                '端到端加密。加密数据可以由我们保管，也可以指向你自己的服务器。密钥永远不会离开你的设备。',
              )}
            </p>
            <p className="tv-onb-card-state">
              {tr(
                'Arrives with the mobile apps — not available yet',
                '将随移动端应用一起推出——目前尚不可用',
              )}
            </p>
          </div>
        </div>
      </section>
      <footer className="tv-onb-foot">
        <button type="button" className="tv-onb-primary" onClick={onContinue}>
          {tr('Continue', '继续')}
          <kbd>↵</kbd>
        </button>
        <span>{tr('No account needed either way.', '两种方式都无需账户。')}</span>
      </footer>
    </>
  );
}

/** Plain Enter advances a step whose primary action carries the ↵ badge. */
export function useEnterAdvances(advance: () => void, enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Enter' || event.metaKey || event.ctrlKey) return;
      const target = event.target as HTMLElement | null;
      // Never steal Enter from a field or an explicitly focused button.
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
      if (target instanceof HTMLButtonElement) return;
      advance();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [advance, enabled]);
}

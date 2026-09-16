// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { onboardingComplete } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { TextAction } from '../../paper/kit';
import { FirstSnippetStep } from './first-snippet-step';
import './onboarding.css';
import { PermissionStep } from './permission-step';
import { STEP_COUNT, StepBody, StepSpine, useEnterAdvances } from './step-kit';
import { TryStep } from './try-step';

interface OnboardingPageProps {
  /** Called after the completion marker is handled; the shell takes over. */
  onDone: () => void;
}

/**
 * First launch in four steps: installed, permission, first words, try it
 * once. Every step can be skipped, and whatever is skipped can still be done
 * in Settings later.
 */
export function OnboardingPage({ onDone }: OnboardingPageProps) {
  const tr = useTr();
  const navigate = useNavigate();
  const [step, setStep] = useState(1);
  const [finishError, setFinishError] = useState(false);

  const finish = (destination?: string) => {
    onboardingComplete()
      .then(() => {
        if (destination !== undefined) void navigate(destination);
        onDone();
      })
      .catch(() => setFinishError(true));
  };

  const next = () => setStep((current) => Math.min(current + 1, STEP_COUNT));

  return (
    <main className="tpi tvo">
      {/* Onboarding renders without the shell, so this bar is what drags the window. */}
      <header className="tvo-bar" data-tauri-drag-region="">
        {tr('Getting started with Typvia', '开始用 Typvia')}
      </header>
      <div className="tvo-sheet">
        <StepSpine step={step} onPick={setStep} />
        {step === 1 && <InstalledStep onContinue={next} onSkip={() => finish()} />}
        {step === 2 && <PermissionStep onContinue={next} />}
        {step === 3 && <FirstSnippetStep onContinue={next} onImport={() => finish('/settings')} />}
        {step === 4 && <TryStep onFinish={() => finish()} />}
        {finishError && (
          <div className="tpi-note tvo-note" role="alert">
            <p className="tpi-note-title">{tr('Your snippets are safe.', '你的片段都好好的。')}</p>
            <p className="tpi-note-body">
              {tr(
                'Setup could not note that it finished, so it may show again next launch.',
                '只是没记下「已经设置过」,下次启动可能还会出现这几步。',
              )}
            </p>
            <div className="tpi-note-actions">
              <button type="button" className="tpi-note-yes" onClick={() => finish()}>
                {tr('Try again', '再试一次')}
              </button>
              <button type="button" className="tpi-note-no" onClick={onDone}>
                {tr('Continue anyway', '先这样继续')}
              </button>
            </div>
          </div>
        )}
      </div>
    </main>
  );
}

function InstalledStep({ onContinue, onSkip }: { onContinue: () => void; onSkip: () => void }) {
  const tr = useTr();
  useEnterAdvances(onContinue);
  return (
    <StepBody
      mascot="happy"
      title={tr('Installed.', '装好了。')}
      actions={
        <>
          <TextAction primary onClick={onContinue}>
            {tr('Start', '开始')}
          </TextAction>
          <TextAction onClick={onSkip}>{tr('Skip setup', '跳过设置')}</TextAction>
        </>
      }
    >
      <p>
        {tr(
          'Three more steps, about two minutes. Every one can be skipped — whatever you skip can still be done in Settings later.',
          '接下来三步,大约两分钟。每一步都可以跳过 —— 跳过的那步以后在设置里还能做。',
        )}
      </p>
      <p>
        {tr(
          'Your snippets live in a local library on this Mac. No account needed; syncing between devices can be turned on in Settings whenever you like.',
          '片段存在这台 Mac 上的本地库里,不需要账户;想在几台设备之间同步,以后在设置里随时能开。',
        )}
      </p>
    </StepBody>
  );
}

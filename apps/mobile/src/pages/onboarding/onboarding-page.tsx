import { onboardingComplete } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { deviceWord, isAndroid } from '../../platform';
import { SnippetEditorPage } from '../editor';
import { DoneStep } from './done-step';
import { FullAccessStep } from './full-access-step';
import { KeyboardStep } from './keyboard-step';
import './onboarding.css';

// iOS carries the Full Access ledger as its own step; Android has no Full
// Access concept — the platform warning is named inside the keyboard step
// instead, so the flow is one step shorter.
const STEP_COUNT = isAndroid ? 5 : 6;
const STEP_COUNT_WORD = isAndroid ? 'five' : 'six';
const STEP_COUNT_WORD_ZH = isAndroid ? '五' : '六';
/** Chinese reading of the platform device word ('phone' / 'iPhone'). */
const deviceWordZh = isAndroid ? '手机' : 'iPhone';

interface OnboardingPageProps {
  /** Called after the completion marker is handled; the shell takes over. */
  onDone: () => void;
  /** A snippet was saved during setup — the host keeps the snapshot in step. */
  onSnippetSaved?: (() => void) | undefined;
}

/**
 * Mobile first-run flow: full-screen pages, one idea per screen, a visible
 * way out on every screen, transitions are cuts. The desktop structure
 * (apps/desktop/src/pages/onboarding) is mirrored, not forked: same
 * skip/finish semantics, same honest-copy patterns. iOS adds the Full
 * Access ledger (six steps); Android ends after the keyboard step (five) —
 * the composition difference lives here, the copy differences inside the
 * platform-aware steps.
 */
export function OnboardingPage({ onDone, onSnippetSaved }: OnboardingPageProps) {
  const tr = useTr();
  const [step, setStep] = useState(1);
  const [finishError, setFinishError] = useState(false);

  const finish = () => {
    onboardingComplete()
      .then(onDone)
      .catch(() => {
        setFinishError(true);
      });
  };

  const forceFinish = () => {
    // The marker write failed and the user chose to continue: nothing is
    // lost — onboarding simply offers itself again next launch.
    onDone();
  };

  const handleSaved = () => {
    onSnippetSaved?.();
    setStep(4);
  };

  const header = (
    <header className="tv-monb-head">
      <span className="tv-monb-step-count">
        {tr(`Step ${step} of ${STEP_COUNT}`, `第 ${step} 步,共 ${STEP_COUNT} 步`)}
      </span>
      {step === 1 && (
        <button type="button" className="tv-monb-escape" onClick={finish}>
          {tr('Skip setup', '跳过设置')}
        </button>
      )}
      {step > 1 && step < STEP_COUNT && (
        <button
          type="button"
          className="tv-monb-escape"
          onClick={() => setStep((s) => Math.max(1, s - 1))}
        >
          {tr('Back', '返回')}
        </button>
      )}
    </header>
  );

  if (step === 3) {
    /*
     * Step 3 reuses the real create page — the product itself is the demo,
     * and a second editor would drift. Cancel is the step's visible skip.
     * The editor renders its own <main>, so this branch keeps the chrome as
     * a plain wrapper instead of nesting landmarks.
     * Desktop parity gap: desktop onboarding prefills from a
     * host-filtered clipboard seed; the mobile host exposes no
     * clipboard-seed command, so the page starts empty here.
     */
    return (
      <div className="tv-monb">
        {header}
        <div className="tv-monb-editor-host">
          <SnippetEditorPage onCancel={() => setStep(4)} onSaved={handleSaved} />
        </div>
      </div>
    );
  }

  return (
    <main className="tv-monb">
      {header}

      {step === 1 && <WelcomeStep onContinue={() => setStep(2)} />}
      {step === 2 && <StorageStep onContinue={() => setStep(3)} />}
      {step === 4 && <KeyboardStep onContinue={() => setStep(5)} />}
      {step === 5 && !isAndroid && <FullAccessStep onContinue={() => setStep(6)} />}
      {step === STEP_COUNT && <DoneStep onFinish={finish} />}

      {finishError && (
        <div className="tv-monb-finish-error" role="alert">
          <span>
            {tr(
              'Your snippets are safe — setup could not record completion, so it may show again next launch.',
              '你的片段安然无恙 —— 设置流程未能记录完成状态,下次启动时可能会再次出现。',
            )}
          </span>
          <div className="tv-monb-finish-error-actions">
            <button type="button" className="tv-monb-ghost" onClick={finish}>
              {tr('Try again', '重试')}
            </button>
            <button type="button" className="tv-monb-ghost" onClick={forceFinish}>
              {tr('Continue anyway', '仍然继续')}
            </button>
          </div>
        </div>
      )}
    </main>
  );
}

interface StepProps {
  onContinue: () => void;
}

/** Step 1 — a statement, not a pitch: the accent-bar wordmark
 * eyebrow, then the tagline in the light display cut. */
function WelcomeStep({ onContinue }: StepProps) {
  const tr = useTr();
  return (
    <>
      <section className="tv-monb-body tv-monb-welcome">
        <div className="tv-monb-eyebrow">
          <span aria-hidden="true" className="tv-monb-eyebrow-bar" />
          <span className="tv-monb-eyebrow-name">Typvia</span>
        </div>
        <h1 className="tv-monb-tagline">
          {tr('Save once.', '保存一次，')}
          <br />
          {tr('Type anywhere.', '随处输入。')}
        </h1>
        <p className="tv-monb-welcome-lead">
          {tr(
            `Typvia keeps the text you retype — replies, addresses, commands, prompts — and turns it into a keyboard. Everything stays on this ${deviceWord} unless you ask otherwise.`,
            `Typvia 保存你反复输入的文字 —— 回复、地址、命令、提示词 —— 并把它们变成一个键盘。除非你主动选择,一切都只留在这台${deviceWordZh}上。`,
          )}
        </p>
      </section>
      <footer className="tv-monb-foot">
        <button type="button" className="tv-monb-primary" onClick={onContinue}>
          {tr(`Set up in ${STEP_COUNT_WORD} steps`, `${STEP_COUNT_WORD_ZH}步完成设置`)}
        </button>
        <span className="tv-monb-foot-note">
          {tr(
            'takes about a minute · open source · local-first · end-to-end encrypted when synced',
            '约一分钟 · 开源 · 本地优先 · 同步时端到端加密',
          )}
        </span>
      </footer>
    </>
  );
}

/** Step 2 — where it lives: local-only first, sync described without persuasion. */
function StorageStep({ onContinue }: StepProps) {
  const tr = useTr();
  return (
    <>
      <section className="tv-monb-body">
        <h1 className="tv-monb-title">{tr('Where should this live?', '内容应该存放在哪里?')}</h1>
        <div className="tv-monb-cards">
          <div className="tv-monb-card is-selected">
            <span aria-hidden="true" className="tv-monb-card-glyph" />
            <h2 className="tv-monb-card-title">{tr(`This ${deviceWord} only`, '仅本机')}</h2>
            <p className="tv-monb-card-body">
              {tr(
                'Nothing ever leaves this phone. No account, no server, no network calls. You can turn on sync any time later.',
                `任何内容都不会离开这台${deviceWordZh}。没有账号、没有服务器、没有网络请求。之后随时可以开启同步。`,
              )}
            </p>
            <p className="tv-monb-card-state">{tr('Selected', '已选择')}</p>
          </div>
          <div className="tv-monb-card is-muted">
            <span aria-hidden="true" className="tv-monb-card-glyph is-chain" />
            <h2 className="tv-monb-card-title">{tr('Sync my devices', '多设备同步')}</h2>
            <p className="tv-monb-card-body">
              {tr(
                'End-to-end encrypted. We can hold the encrypted blobs, or point it at your own server. Keys never leave your devices.',
                '端到端加密。加密数据可以由我们保管,也可以指向你自己的服务器。密钥永远不会离开你的设备。',
              )}
            </p>
            <p className="tv-monb-card-state">
              {tr('Not available on mobile yet', '移动端暂不可用')}
            </p>
          </div>
        </div>
      </section>
      <footer className="tv-monb-foot">
        <button type="button" className="tv-monb-primary" onClick={onContinue}>
          {tr('Continue', '继续')}
        </button>
        <span className="tv-monb-foot-note">
          {tr('No account needed either way.', '两种方式都不需要账号。')}
        </span>
      </footer>
    </>
  );
}

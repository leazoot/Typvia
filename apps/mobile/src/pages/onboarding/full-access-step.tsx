import { useTr } from '@typvia/ui';

interface FullAccessStepProps {
  onContinue: () => void;
}

/**
 * Step 5 — the Full Access ledger: iOS adds this step to name Apple's
 * generic warning before iOS shows it. Honest reduction of the design's
 * three-row ledger: nothing in this release actually needs Full
 * Access, so the "needs it" column says exactly that instead of inventing
 * gated features.
 */
export function FullAccessStep({ onContinue }: FullAccessStepProps) {
  const tr = useTr();
  return (
    <>
      <section className="tv-monb-body">
        <h1 className="tv-monb-title">{tr('About Full Access.', '关于完全访问。')}</h1>

        <h2 className="tv-monb-ledger-label">
          {tr('Works now — without Full Access', '现在就能用 —— 无需完全访问')}
        </h2>
        <ul
          className="tv-monb-ledger"
          aria-label={tr('Works without Full Access', '无需完全访问即可使用')}
        >
          <li className="tv-monb-ledger-row">
            <span className="tv-monb-ledger-title">
              {tr('The keyboard shows your snippets', '键盘会显示你的片段')}
            </span>
            <span className="tv-monb-ledger-sub">
              {tr(
                'From an on-device snapshot — starred and recent first.',
                '来自本机快照 —— 星标和最近使用的排在前面。',
              )}
            </span>
          </li>
          <li className="tv-monb-ledger-row">
            <span className="tv-monb-ledger-title">
              {tr('Tap one, it types itself', '点一下,它会自动输入')}
            </span>
            <span className="tv-monb-ledger-sub">
              {tr('Into whichever app you are in.', '输入到你正在使用的任何应用里。')}
            </span>
          </li>
          <li className="tv-monb-ledger-row">
            <span className="tv-monb-ledger-title">{tr('Works offline', '离线可用')}</span>
            <span className="tv-monb-ledger-sub">
              {tr('The keyboard never makes a network call.', '键盘从不发起网络请求。')}
            </span>
          </li>
        </ul>

        <h2 className="tv-monb-ledger-label">{tr('Needs Full Access', '需要完全访问')}</h2>
        <ul className="tv-monb-ledger" aria-label={tr('Needs Full Access', '需要完全访问')}>
          <li className="tv-monb-ledger-row">
            <span className="tv-monb-ledger-title">{tr('Nothing, today', '目前没有')}</span>
            <span className="tv-monb-ledger-sub">
              {tr(
                "Typvia currently requests no Full Access — the keyboard's core use never needs the toggle. This page exists so you know that before iOS warns you.",
                'Typvia 目前不申请完全访问 —— 键盘的核心功能永远不需要这个开关。这一页只是让你在 iOS 弹出警告之前先了解这一点。',
              )}
            </span>
          </li>
        </ul>

        <p className="tv-monb-lead">
          {tr(
            'If iOS ever shows its warning that a keyboard “may be able to transmit what you type”, that text is generic and applies to every third-party keyboard. Typvia’s keyboard has no analytics and no network calls — the source is public, so this is checkable.',
            '如果 iOS 显示"键盘可能会传输你键入的内容"的警告,那是针对所有第三方键盘的通用文案。Typvia 的键盘没有任何统计分析,也没有网络请求 —— 源代码公开,可以自行验证。',
          )}
        </p>
      </section>
      <footer className="tv-monb-foot">
        <button type="button" className="tv-monb-primary" onClick={onContinue}>
          {tr('Continue', '继续')}
        </button>
        <span className="tv-monb-foot-note">
          {tr(
            'There is no toggle to set — this was the ledger.',
            '没有需要设置的开关 —— 这页只是把账目摆清楚。',
          )}
        </span>
      </footer>
    </>
  );
}

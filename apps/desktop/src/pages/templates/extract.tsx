import {
  aiExtractVariables,
  aiProviderList,
  ipcErrorCopy,
  type VariableProposal,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { chooseProvider, rememberProvider } from '@typvia/ui/ai';

/**
 * AI variable extraction, living in the builder's "Mark what changes"
 * column — the design's "AI-extracted variables" state. Proposals are
 * per-span and individually applicable;
 * applying only rewrites the builder's local body/fields state, and
 * nothing persists until the user presses the builder's own Save.
 */

type Phase =
  | { at: 'idle' }
  | { at: 'loading' }
  | { at: 'unconfigured' }
  | { at: 'error'; message: string }
  | { at: 'ready'; proposals: VariableProposal[] };

interface ExtractProps {
  body: string;
  /** Applies one proposal into the builder's local body/fields state. */
  onApply: (proposal: VariableProposal) => void;
}

export function Extract({ body, onApply }: ExtractProps) {
  const tr = useTr();
  const navigate = useNavigate();
  const [phase, setPhase] = useState<Phase>({ at: 'idle' });
  const [applied, setApplied] = useState<string[]>([]);

  const run = async () => {
    setPhase({ at: 'loading' });
    setApplied([]);
    try {
      const provider = chooseProvider(await aiProviderList());
      if (provider === null) {
        setPhase({ at: 'unconfigured' });
        return;
      }
      rememberProvider(provider.id);
      const proposals = await aiExtractVariables(provider.id, { body, isSensitive: false });
      setPhase({ at: 'ready', proposals });
    } catch (caught) {
      const message =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the extraction failed', '提取失败');
      setPhase({ at: 'error', message });
    }
  };

  if (phase.at === 'idle') {
    return (
      <div className="tv-builder-ai">
        <button
          type="button"
          className="tv-builder-ai-run"
          disabled={body.trim() === ''}
          onClick={() => void run()}
        >
          {tr('Extract variables', 'AI 提取变量')}
        </button>
      </div>
    );
  }

  if (phase.at === 'loading') {
    return (
      <div className="tv-builder-ai" aria-busy="true">
        <span className="tv-builder-ai-note" role="status">
          {tr('Reading the text…', '正在阅读文本…')}
        </span>
      </div>
    );
  }

  if (phase.at === 'unconfigured') {
    return (
      <div className="tv-builder-ai">
        <span className="tv-builder-ai-note" role="status">
          {tr(
            'Marking by hand still works — AI extraction needs a provider.',
            '手动标记仍然可用——AI 提取需要先配置一个 Provider。',
          )}
        </span>
        <button
          type="button"
          className="tv-builder-ai-run"
          onClick={() => void navigate('/settings')}
        >
          {tr('Set up in Settings', '前往设置配置')}
        </button>
        <button
          type="button"
          className="tv-builder-ai-quiet"
          onClick={() => setPhase({ at: 'idle' })}
        >
          {tr('Dismiss', '关闭')}
        </button>
      </div>
    );
  }

  if (phase.at === 'error') {
    return (
      <div className="tv-builder-ai">
        <span className="tv-builder-ai-note" role="status">
          {tr(`Your text is unchanged — ${phase.message}`, `你的文本没有改动——${phase.message}`)}
        </span>
        <button type="button" className="tv-builder-ai-run" onClick={() => void run()}>
          {tr('Try again', '重试')}
        </button>
        <button
          type="button"
          className="tv-builder-ai-quiet"
          onClick={() => setPhase({ at: 'idle' })}
        >
          {tr('Dismiss', '关闭')}
        </button>
      </div>
    );
  }

  const open = phase.proposals.filter((p) => !applied.includes(p.name));

  return (
    <div className="tv-builder-ai" role="group" aria-label={tr('Variable proposals', '变量建议')}>
      {phase.proposals.length === 0 ? (
        <span className="tv-builder-ai-note" role="status">
          {tr(
            'Nothing to extract — this text may not need variables.',
            '没有可提取的内容——这段文本可能不需要变量。',
          )}
        </span>
      ) : (
        <>
          <span className="tv-builder-ai-note" role="status">
            {open.length === 0
              ? tr('All proposals applied.', '所有建议均已应用。')
              : tr(
                  'Proposals — apply the ones that should vary:',
                  '提取建议——应用其中应当变化的部分:',
                )}
          </span>
          {open.map((proposal) => (
            <button
              key={proposal.name}
              type="button"
              className="tv-builder-ai-field"
              onClick={() => {
                onApply(proposal);
                setApplied((names) => [...names, proposal.name]);
              }}
            >
              <span className="tv-builder-ai-field-label">{`{{${proposal.name}}}`}</span>
              <span className="tv-builder-ai-field-value">{proposal.original}</span>
            </button>
          ))}
        </>
      )}
      <button
        type="button"
        className="tv-builder-ai-quiet"
        onClick={() => setPhase({ at: 'idle' })}
      >
        {tr('Dismiss', '关闭')}
      </button>
    </div>
  );
}

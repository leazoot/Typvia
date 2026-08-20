import { type ActionRunInput, aiActionRun, ipcErrorCopy } from '@typvia/shared';
import { useState } from 'react';
import { useTr } from '../i18n';
import { countWords } from './action-labels';

/**
 * One action run as both hosts stage it: running → a pending result that
 * applies nothing until the user confirms. Applying is the host's job (the
 * targets differ — a try-it pane on desktop, the editor body on mobile), so
 * the hook only owns the state machine; the host reports the outcome back
 * through `applied` / `failed`.
 */
export type ActionRun =
  | { at: 'idle' }
  | { at: 'running' }
  | { at: 'error'; message: string }
  | { at: 'pending'; output: string; maskedKinds: string[]; seconds: string; words: number }
  | { at: 'applied'; note: string };

export function useActionRun() {
  const tr = useTr();
  const [run, setRun] = useState<ActionRun>({ at: 'idle' });

  const start = async (actionId: string, input: ActionRunInput): Promise<void> => {
    setRun({ at: 'running' });
    const started = performance.now();
    try {
      const result = await aiActionRun(actionId, input);
      setRun({
        at: 'pending',
        output: result.output,
        maskedKinds: result.maskedKinds,
        seconds: ((performance.now() - started) / 1000).toFixed(1),
        words: countWords(input.text),
      });
    } catch (caught) {
      const message =
        caught instanceof Error ? tr(...ipcErrorCopy(caught)) : tr('the run failed', '运行失败');
      setRun({ at: 'error', message });
    }
  };

  const applied = (note: string) => setRun({ at: 'applied', note });
  const failed = (message: string) => setRun({ at: 'error', message });
  const dismiss = () => setRun({ at: 'idle' });

  return { run, start, applied, failed, dismiss };
}

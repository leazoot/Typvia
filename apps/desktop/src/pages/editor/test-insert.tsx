import { IpcError, copySnippet, injectSnippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';

/** Seconds the countdown gives the user to focus their target app. */
const COUNTDOWN_START = 3;
/** How long the result label lingers before the control resets. */
const RESULT_MS = 1600;

type Phase =
  | { kind: 'idle' }
  | { kind: 'counting'; remaining: number }
  | { kind: 'working' }
  | { kind: 'done'; label: string };

interface TestInsertProps {
  /** The saved snippet id, or null while the draft has never been saved. */
  snippetId: string | null;
  /** True only when the saved content matches the editor (safe to inject). */
  ready: boolean;
}

/**
 * A working "test insert": there is no in-app target, so a short countdown
 * lets the user focus their real target window before the text is injected
 * into the frontmost app (the design has no countdown spec — this follows the
 * status-line language, recorded as a deviation). Falls back to copying when
 * the OS has not granted injection permission.
 */
export function TestInsert({ snippetId, ready }: TestInsertProps) {
  const tr = useTr();
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);

  const clearTimers = () => {
    for (const timer of timers.current) clearTimeout(timer);
    timers.current = [];
  };
  useEffect(() => clearTimers, []);

  const later = (fn: () => void, ms: number) => {
    timers.current.push(setTimeout(fn, ms));
  };

  const finish = (label: string) => {
    setPhase({ kind: 'done', label });
    later(() => {
      setPhase({ kind: 'idle' });
    }, RESULT_MS);
  };

  const run = (id: string) => {
    setPhase({ kind: 'working' });
    injectSnippet(id)
      .then(() => {
        finish(tr('Inserted', '已插入'));
      })
      .catch((error: unknown) => {
        if (error instanceof IpcError && error.code === 'permission_denied') {
          // Degrade to copy when the OS won't let us synthesize keystrokes.
          copySnippet(id).then(
            () => {
              finish(tr('Copied instead', '已改为复制'));
            },
            () => {
              finish(tr("Couldn't insert", '未能插入'));
            },
          );
        } else {
          finish(tr("Couldn't insert", '未能插入'));
        }
      });
  };

  const startCountdown = (id: string) => {
    let remaining = COUNTDOWN_START;
    setPhase({ kind: 'counting', remaining });
    const tick = () => {
      remaining -= 1;
      if (remaining <= 0) {
        run(id);
      } else {
        setPhase({ kind: 'counting', remaining });
        later(tick, 1000);
      }
    };
    later(tick, 1000);
  };

  const onClick = () => {
    if (phase.kind === 'counting') {
      clearTimers();
      setPhase({ kind: 'idle' });
      return;
    }
    if (phase.kind === 'idle' && snippetId !== null) {
      startCountdown(snippetId);
    }
  };

  const label =
    phase.kind === 'counting'
      ? tr('Cancel', '取消')
      : phase.kind === 'working'
        ? tr('Inserting…', '插入中…')
        : phase.kind === 'done'
          ? phase.label
          : tr('Test insert', '测试插入');

  return (
    <div className="tv-ed-testinsert">
      <button
        type="button"
        className="tv-ed-testinsert-btn"
        disabled={(!ready && phase.kind === 'idle') || phase.kind === 'working'}
        onClick={onClick}
      >
        {label}
      </button>
      {phase.kind === 'counting' && (
        <span role="status" className="tv-ed-testinsert-hint">
          {tr(
            `Focus your target app… ${String(phase.remaining)}`,
            `请聚焦目标应用… ${String(phase.remaining)}`,
          )}
        </span>
      )}
    </div>
  );
}

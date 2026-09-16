// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { useEffect, useId, type ReactNode } from 'react';
import { Mascot, type MascotState } from '../../paper/mascot';

export const STEP_COUNT = 4;

/** Plain Enter moves on from a step whose primary action allows it. */
export function useEnterAdvances(advance: () => void, enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Enter' || event.metaKey || event.ctrlKey) return;
      const target = event.target;
      // Never take Enter from a field or from a button that has the focus.
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLButtonElement
      ) {
        return;
      }
      advance();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [advance, enabled]);
}

/** One step: the mascot in the mood of the step, a serif line, the words, the ways on. */
export function StepBody({
  mascot,
  title,
  children,
  actions,
}: {
  mascot: MascotState;
  title: string;
  children: ReactNode;
  actions: ReactNode;
}) {
  return (
    <section className="tvo-step">
      <Mascot state={mascot} size={72} />
      <div className="tvo-step-main">
        <h1 className="tvo-title">{title}</h1>
        <div className="tvo-text">{children}</div>
        <div className="tvo-actions">{actions}</div>
      </div>
    </section>
  );
}

const NODES = [
  { x: 30, y: 52 },
  { x: 320, y: 44 },
  { x: 610, y: 46 },
  { x: 850, y: 38 },
];

const SEGMENTS = [
  'M30 52C140 20 230 20 320 44',
  'M320 44C420 70 520 70 610 46',
  'M610 46C700 24 780 24 850 38',
];

/**
 * Where the reader is, as a line with four stops: solid behind, dotted 2/9
 * ahead — never a row of dots. A stop already passed takes you back to it.
 */
export function StepSpine({ step, onPick }: { step: number; onPick: (step: number) => void }) {
  const tr = useTr();
  const gradient = useId();
  const names = [
    tr('Installed', '装好了'),
    tr('Permission', '授权'),
    tr('First words', '第一句话'),
    tr('Try it once', '试着打一次'),
  ];
  const current = names[step - 1] ?? '';

  return (
    <svg
      className="tvo-spine"
      viewBox="0 0 880 96"
      role="group"
      aria-label={tr(
        `Step ${String(step)} of ${String(STEP_COUNT)} · ${current}`,
        `第 ${String(step)} 步,共 ${String(STEP_COUNT)} 步 · ${current}`,
      )}
    >
      <defs>
        <linearGradient id={gradient} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" className="tvo-stop-a" />
          <stop offset="1" className="tvo-stop-b" />
        </linearGradient>
      </defs>
      {SEGMENTS.map((path, index) =>
        index + 1 < step ? (
          <path key={path} d={path} stroke={`url(#${gradient})`} className="tvo-spine-path" />
        ) : (
          <path key={path} d={path} className="tvo-spine-path is-ahead" />
        ),
      )}
      {NODES.map((node, index) => {
        const number = index + 1;
        const name = names[index] ?? '';
        const anchor = index === 0 ? 'start' : index === NODES.length - 1 ? 'end' : 'middle';
        const labelY = node.y + 28;
        if (number === step) {
          return (
            <g key={number}>
              <circle cx={node.x} cy={node.y} r="12" className="tvo-spine-halo" />
              <circle cx={node.x} cy={node.y} r="7" className="tvo-spine-now" />
              <text x={node.x} y={labelY} textAnchor={anchor} className="tvo-spine-label">
                {tr(`${name} · step ${String(number)}`, `${name} · 第 ${String(number)} 步`)}
              </text>
            </g>
          );
        }
        if (number < step) {
          return (
            <g
              key={number}
              role="button"
              tabIndex={0}
              aria-label={tr(`Back to ${name}`, `回到「${name}」`)}
              className="tvo-spine-back"
              onClick={() => onPick(number)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onPick(number);
                }
              }}
            >
              <circle cx={node.x} cy={node.y} r="4" className="tvo-spine-done" />
              <text x={node.x} y={labelY} textAnchor={anchor} className="tvo-spine-label">
                {name}
              </text>
            </g>
          );
        }
        return (
          <g key={number}>
            <circle cx={node.x} cy={node.y} r="3.5" className="tvo-spine-ahead" />
            <text x={node.x} y={labelY} textAnchor={anchor} className="tvo-spine-label is-ahead">
              {name}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

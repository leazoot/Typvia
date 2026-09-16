// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';

export type ListStateKind = 'loading' | 'failed' | 'empty-library' | 'empty-collection';

/**
 * The list column when there is no list to show. Loading is the mascot
 * thinking, never a spinner; failure says the snippets are still here before
 * it asks for anything.
 */
export function ListState({
  kind,
  onNew,
  onRetry,
}: {
  kind: ListStateKind;
  onNew: () => void;
  onRetry: () => void;
}) {
  const tr = useTr();

  if (kind === 'loading') {
    return (
      <div className="tvl-state" role="status">
        <div className="tvl-state-line">
          <Mascot state="thinking" size={46} />
          <span className="tvl-state-meta">
            {tr('Reading your snippets into memory', '正在把片段读进内存')}
          </span>
        </div>
      </div>
    );
  }

  if (kind === 'failed') {
    return (
      <div className="tvl-state">
        <div className="tvl-state-line">
          <Mascot state="confused" size={42} />
          <p className="tvl-state-title">
            {tr('Your snippets are all still here.', '你的片段都还在。')}
          </p>
        </div>
        <p className="tvl-state-body">
          {tr(
            "This column didn't load just now; nothing on this Mac was touched.",
            '这一栏刚才没读出来,本机的数据一点没动。',
          )}
        </p>
        <TextAction primary onClick={onRetry}>
          {tr('Read it again', '再读一次')}
        </TextAction>
      </div>
    );
  }

  if (kind === 'empty-collection') {
    return (
      <div className="tvl-state">
        <p className="tvl-state-title">{tr('This collection is empty.', '这个集合还是空的。')}</p>
        <p className="tvl-state-body">
          {tr(
            'Pick it as the collection when you write a snippet and it appears here.',
            '新建片段时把集合选成它,片段就会出现在这里。',
          )}
        </p>
        <TextAction primary onClick={onNew}>
          {tr('New snippet', '新建片段')}
        </TextAction>
      </div>
    );
  }

  return (
    <div className="tvl-state is-empty">
      <span aria-hidden="true" className="tvl-drift is-a">
        Thanks for flagging
      </span>
      <span aria-hidden="true" className="tvl-drift is-b">
        收到,明天给你
      </span>
      <Mascot size={58} />
      <p className="tvl-state-hero">{tr('Nothing of yours here yet.', '还没有属于你的片段。')}</p>
      <p className="tvl-state-body">
        {tr(
          'Start with the sentence you just typed for the second time today — press ⌘⇧N, write it down, and it is in.',
          '从今天你重复打了第二遍的那句话开始 —— 按 ⌘⇧N 写下它,它就进来了。',
        )}
      </p>
      <TextAction primary onClick={onNew}>
        {tr('Write the first snippet', '新建第一条片段')}
      </TextAction>
    </div>
  );
}

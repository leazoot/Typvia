// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  type AiAction,
  aiActionDelete,
  aiActionList,
  aiActionSave,
  type AiProvider,
  aiProviderList,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { GroupTitle, TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';
import { usePaperMenu } from '../../paper/menu';
import { ActionComposer, type SaveState } from './action-composer';
import { type ActionDraft, saveInput, toDraft } from './action-draft';
import { NewAction } from './action-new';
import './actions.css';

const AUTOSAVE_MS = 700;

type Load =
  | { at: 'loading' }
  | { at: 'error' }
  | { at: 'off' }
  | { at: 'ready'; actions: AiAction[]; providers: AiProvider[] };

function firstLine(prompt: string): string {
  const line = prompt.split('\n').find((row) => row.trim() !== '') ?? '';
  return line.length > 64 ? `${line.slice(0, 64)}…` : line;
}

/**
 * AI actions: the actions down the left, one at a time on the page. Each
 * states its flow as a sentence of words that open their choices; the
 * instruction is the page's one boxed field; it saves itself.
 */
export function ActionsPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const location = useLocation();
  const rowMenu = usePaperMenu(tr('Action options', '动作选项'));
  const [load, setLoad] = useState<Load>({ at: 'loading' });
  const [draft, setDraft] = useState<ActionDraft | null>(null);
  const [composing, setComposing] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>('idle');
  const dirty = useRef(false);

  const reload = useCallback(async (selectId?: string) => {
    try {
      const [actions, providers] = await Promise.all([aiActionList(), aiProviderList()]);
      if (providers.length === 0) {
        setLoad({ at: 'off' });
        setDraft(null);
        return;
      }
      setLoad({ at: 'ready', actions, providers });
      const selected =
        selectId === undefined ? actions[0] : actions.find((action) => action.id === selectId);
      setDraft(selected === undefined ? null : toDraft(selected));
    } catch {
      setLoad({ at: 'error' });
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  // "New AI action" from elsewhere arrives with an intent in the router state.
  useEffect(() => {
    if (
      typeof location.state === 'object' &&
      location.state !== null &&
      'create' in location.state
    ) {
      setComposing(true);
    }
  }, [location.state]);

  /** Refreshes the list without disturbing the composer. */
  const refreshList = useCallback(async () => {
    try {
      const actions = await aiActionList();
      setLoad((current) => (current.at === 'ready' ? { ...current, actions } : current));
    } catch {
      // The list stays as it was; the composer is the source of truth here.
    }
  }, []);

  const edit = (next: ActionDraft) => {
    dirty.current = true;
    setDraft(next);
  };

  // A draft without a name is not yet an action, so it waits.
  useEffect(() => {
    if (!dirty.current || draft === null || draft.name.trim() === '') return;
    const timer = setTimeout(() => {
      dirty.current = false;
      setSaveState('saving');
      aiActionSave(saveInput(draft))
        .then(async (saved) => {
          setSaveState('saved');
          setDraft((current) =>
            current === null || current.id !== null ? current : { ...current, id: saved.id },
          );
          await refreshList();
        })
        .catch(() => setSaveState('error'));
    }, AUTOSAVE_MS);
    return () => clearTimeout(timer);
  }, [draft, refreshList]);

  useEffect(() => {
    if (saveState !== 'saved') return;
    const timer = setTimeout(() => setSaveState('idle'), 1600);
    return () => clearTimeout(timer);
  }, [saveState]);

  if (load.at === 'loading') {
    return (
      <div className="tpi tva-single" aria-busy="true">
        <Mascot state="thinking" size={58} />
      </div>
    );
  }

  if (load.at === 'error') {
    return (
      <div className="tpi tva-single">
        <Mascot state="confused" size={58} />
        <div>
          <h1>{tr('Your actions are safe.', '你的动作都还在。')}</h1>
          <p>{tr('The list just could not load.', '只是列表暂时读不出来。')}</p>
          <TextAction primary onClick={() => void reload()}>
            {tr('Try again', '再试一次')}
          </TextAction>
        </div>
      </div>
    );
  }

  if (load.at === 'off') {
    return (
      <div className="tpi tva-single">
        <Mascot state="sleep" size={58} />
        <div>
          <h1>{tr('Everything else works without AI.', '没有 AI 也不影响其他功能。')}</h1>
          <p>
            {tr(
              'Actions need one provider — your own key, or a model on this Mac.',
              'AI 动作需要先接一个服务方——你自己的 Key,或者这台 Mac 上的模型。',
            )}
          </p>
          <TextAction primary onClick={() => void navigate('/settings')}>
            {tr('Set up in Settings', '去设置里接一个')}
          </TextAction>
        </div>
      </div>
    );
  }

  const { actions, providers } = load;

  const duplicate = (action: AiAction) => {
    void aiActionSave({
      ...saveInput(toDraft(action)),
      id: null,
      name: tr(`${action.name} copy`, `${action.name} 副本`),
    }).then((saved) => reload(saved.id));
  };

  return (
    <div className="tpi tva">
      <aside className="tva-list" aria-label={tr('Actions', '动作')}>
        <GroupTitle
          trailing={
            <TextAction
              onClick={() => {
                setComposing(true);
                setDraft(null);
              }}
            >
              {tr('New', '新建')}
            </TextAction>
          }
        >
          {tr('Actions', '动作')}
        </GroupTitle>
        {actions.length === 0 && (
          <p className="tva-note">{tr('No actions yet.', '还没有动作。')}</p>
        )}
        {actions.map((action) => (
          <button
            key={action.id}
            type="button"
            className="tva-item"
            aria-current={!composing && action.id === draft?.id}
            onClick={() => {
              setComposing(false);
              dirty.current = false;
              setDraft(toDraft(action));
            }}
            onContextMenu={(event) =>
              rowMenu.openAtPointer(event, [
                {
                  kind: 'item',
                  label: tr('Duplicate', '创建副本'),
                  onSelect: () => duplicate(action),
                },
                { kind: 'separator' },
                {
                  kind: 'item',
                  label: tr('Delete', '删除'),
                  danger: true,
                  onSelect: () => void aiActionDelete(action.id).then(() => reload()),
                },
              ])
            }
          >
            <span className="tva-item-name">{action.name}</span>
            <span className="tva-item-desc">{firstLine(action.promptTemplate)}</span>
          </button>
        ))}
      </aside>

      <section className="tva-composer" aria-label={tr('Action composer', '动作编辑')}>
        {composing || draft === null ? (
          <NewAction
            onCreate={(name, prompt) => {
              setComposing(false);
              dirty.current = true;
              setDraft({
                id: null,
                name,
                promptTemplate: prompt,
                providerId: providers[0]?.id ?? null,
                model: '',
                inputSource: 'selection',
                outputMode: 'copy',
                permissionScope: 'normal_only',
                temperature: '',
                isBuiltin: false,
              });
            }}
          />
        ) : (
          <ActionComposer
            draft={draft}
            providers={providers}
            saveState={saveState}
            onEdit={edit}
            onDuplicate={() => {
              const current = actions.find((action) => action.id === draft.id);
              if (current !== undefined) duplicate(current);
            }}
            onDeleted={() => void reload()}
          />
        )}
      </section>
      {rowMenu.node}
    </div>
  );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { getSnippet, templateFields } from '@typvia/shared';
import type { Folder, Snippet, TemplateField } from '@typvia/shared';
import { useLocale, useTr, type Tr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router';
import { useEspanso } from '../../espanso/espanso-context';
import { TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';
import { useTitleBar } from '../../workspace/title-bar';
import { fetchFolders } from '../library/library-data';
import { isCode } from '../library/library-model';
import { variableNames } from './editor-body';
import { BodyBlock, LineFields, VariableDefs } from './editor-fields';
import { EditorMore } from './editor-more';
import { Specimen } from './editor-specimen';
import { useEditorDraft, type SaveStatus } from './use-editor-draft';
import './editor.css';

function statusText(
  status: SaveStatus,
  version: number | null,
  savedAt: number | null,
  tr: Tr,
): string {
  if (status === 'saving') return tr('Saving…', '正在写进本地库…');
  if (status === 'error') return tr('Not saved', '没存上');
  if (status === 'draft' || version === null) return tr('Draft', '草稿');
  const age = savedAt === null ? null : Date.now() - savedAt;
  const when =
    age === null || age < 60_000
      ? tr('just now', '刚刚')
      : age < 3_600_000
        ? tr(
            `${String(Math.floor(age / 60_000))} min ago`,
            `${String(Math.floor(age / 60_000))} 分钟前`,
          )
        : tr('earlier', '更早');
  return tr(
    `Saved · v${String(version)} · ${when}`,
    `写进本地库了 · v${String(version)} · ${when}`,
  );
}

/** Reads a seeded draft value handed over by another screen's navigation. */
function seededValue(state: unknown, key: 'draftTitle' | 'draftTrigger' | 'draftBody'): string {
  if (typeof state === 'object' && state !== null && key in state) {
    const value = (state as Record<string, unknown>)[key];
    if (typeof value === 'string') return value;
  }
  return '';
}

function wantsTrigger(state: unknown): boolean {
  return (
    typeof state === 'object' &&
    state !== null &&
    'focus' in state &&
    (state as Record<string, unknown>).focus === 'trigger'
  );
}

interface Seed {
  title: string;
  trigger: string | null;
  body: string;
}

function LoadedEditor({
  initial,
  seed,
  folders,
  focusTrigger,
}: {
  initial: Snippet | null;
  seed: Seed;
  folders: readonly Folder[];
  focusTrigger: boolean;
}) {
  const tr = useTr();
  const locale = useLocale();
  const navigate = useNavigate();
  const { draft, patch, status, errorMessage, version, savedAt, sensitiveKinds } = useEditorDraft(
    initial,
    seed.title,
    seed.trigger,
    seed.body,
  );

  // A saved snippet may add or change a trigger, so keep the espanso config in
  // sync (debounced, and only when the integration is on). Seeded with the
  // initial save time so opening an existing snippet does not trigger a sync.
  const { notifyMutation } = useEspanso();
  const lastSyncedAt = useRef<number | null>(savedAt);
  useEffect(() => {
    if (status === 'saved' && savedAt !== null && savedAt !== lastSyncedAt.current) {
      lastSyncedAt.current = savedAt;
      notifyMutation();
    }
  }, [status, savedAt, notifyMutation]);

  const [fields, setFields] = useState<TemplateField[]>([]);
  useEffect(() => {
    if (draft.id === null) return;
    templateFields(draft.id)
      .then(setFields)
      .catch(() => setFields([]));
  }, [draft.id]);

  useTitleBar(
    {
      trailing: (
        <span className="tve-titlebar-note">
          {tr('Changes go into the local library as you type', '改动随手就写进本地库')}
        </span>
      ),
    },
    locale,
  );

  const names = variableNames(draft.body);
  return (
    <div className="tpi tve">
      <div className="tve-main">
        <LineFields
          draft={draft}
          onPatch={patch}
          status={status}
          errorMessage={errorMessage}
          focusTrigger={focusTrigger}
        />
        <BodyBlock draft={draft} onPatch={patch} variables={names} code={isCode(draft)} />
        <div className="tve-rule" />
        <VariableDefs
          names={names}
          fields={fields}
          canOpenBuilder={draft.id !== null}
          onOpenBuilder={() => draft.id !== null && void navigate(`/templates/${draft.id}`)}
        />
        <EditorMore
          draft={draft}
          onPatch={patch}
          folders={folders}
          sensitiveKinds={sensitiveKinds}
          saved={status === 'saved'}
        />
      </div>
      <Specimen
        body={draft.body}
        names={names}
        fields={fields}
        status={status}
        statusText={statusText(status, version, savedAt, tr)}
        snippetId={draft.id}
        ready={status === 'saved' && draft.body.trim() !== ''}
        onOpenHistory={
          draft.id !== null && version !== null
            ? () => void navigate(`/editor/${draft.id ?? ''}/history`)
            : null
        }
      />
    </div>
  );
}

/**
 * Editing one snippet: trigger and title as lines, the body in the one boxed
 * field, the variables beneath it, and on the right the specimen of what it
 * will type. There is no save button — every change goes into the local
 * library a moment after it is made.
 */
export function EditorPage() {
  const tr = useTr();
  const { id } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const [initial, setInitial] = useState<Snippet | null>(null);
  const [ready, setReady] = useState(id === undefined);
  const [loadFailed, setLoadFailed] = useState(false);
  const [folders, setFolders] = useState<Folder[]>([]);

  useEffect(() => {
    fetchFolders()
      .then(setFolders)
      .catch(() => setFolders([]));
  }, []);

  useEffect(() => {
    if (id === undefined) return;
    getSnippet(id)
      .then((snippet) => {
        setInitial(snippet);
        setReady(true);
      })
      .catch(() => setLoadFailed(true));
  }, [id]);

  if (loadFailed) {
    return (
      <div className="tpi tve-missing">
        <Mascot state="confused" size={42} />
        <p>
          {tr(
            "Your library is intact — this snippet just isn't available.",
            '资料库完好,只是这条片段现在打不开。',
          )}
        </p>
        <TextAction onClick={() => void navigate('/')}>
          {tr('Back to the library', '回到资料库')}
        </TextAction>
      </div>
    );
  }
  if (!ready) return <div className="tpi tve" aria-busy="true" />;
  // Sensitive snippets never open here (the backend refuses the save; the
  // vault owns viewing and editing them).
  if (initial !== null && initial.securityLevel === 'sensitive') {
    return (
      <div className="tpi tve-missing">
        <Mascot state="locked" size={42} />
        <p>
          {tr(
            'Still safely encrypted — secret snippets are viewed and edited in the Vault.',
            '内容仍安全地加密着 —— 密钥片段在保险库里看和改。',
          )}
        </p>
        <TextAction primary onClick={() => void navigate('/vault')}>
          {tr('Open Vault', '去保险库')}
        </TextAction>
      </div>
    );
  }
  return (
    <LoadedEditor
      initial={initial}
      seed={{
        title: seededValue(location.state, 'draftTitle'),
        trigger: seededValue(location.state, 'draftTrigger') || null,
        body: seededValue(location.state, 'draftBody'),
      }}
      folders={folders}
      focusTrigger={wantsTrigger(location.state)}
    />
  );
}

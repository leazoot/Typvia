// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Destructive actions do not ask first. They happen, and a note pinned in the
 * corner says what happened and how to take it back — ⌘Z, or the word on the
 * note. Only the most recent action can be taken back.
 */
import { useTr } from '@typvia/ui';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { editsText } from '../backspace-guard';
import { Note } from '../paper/kit';
import { Mascot, type MascotState } from '../paper/mascot';
import './undo.css';

export interface UndoOffer {
  /** What just happened, e.g. "Deleted /mtg." */
  title: string;
  /** How to take it back, and what else still holds. */
  body: string;
  /** Said if taking it back fails; must say what is still safe. */
  failure: string;
  undo: () => Promise<void>;
}

/** The mascot on a note: it went through, or it did not; `null` keeps the note plain. */
export type NoteMood = Extract<MascotState, 'happy' | 'confused'> | null;

interface PinnedNote {
  title: string;
  body: string;
  undoable: boolean;
  mood: NoteMood;
}

interface UndoContextValue {
  offer: (offer: UndoOffer) => void;
  /**
   * Pins a note with nothing to take back. By default it says something did
   * not happen, and what still holds; pass `happy` when it went through.
   */
  say: (note: { title: string; body: string; mood?: NoteMood }) => void;
  undoLast: () => void;
  canUndo: boolean;
}

const UndoContext = createContext<UndoContextValue>({
  offer: () => undefined,
  say: () => undefined,
  undoLast: () => undefined,
  canUndo: false,
});

/** How long the note stays pinned; the action stays undoable after it goes. */
export const NOTE_MS = 10_000;

export function UndoProvider({ children }: { children: ReactNode }) {
  const tr = useTr();
  const [last, setLast] = useState<UndoOffer | null>(null);
  const [note, setNote] = useState<PinnedNote | null>(null);
  const latest = useRef<UndoOffer | null>(null);
  latest.current = last;
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const pin = useCallback((next: PinnedNote) => {
    setNote(next);
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => setNote(null), NOTE_MS);
  }, []);
  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  const offer = useCallback(
    (next: UndoOffer) => {
      setLast(next);
      pin({ title: next.title, body: next.body, undoable: true, mood: null });
    },
    [pin],
  );

  const say = useCallback(
    ({ title, body, mood = 'confused' }: { title: string; body: string; mood?: NoteMood }) =>
      pin({ title, body, undoable: false, mood }),
    [pin],
  );

  const undoLast = useCallback(() => {
    const current = latest.current;
    if (current === null) return;
    setLast(null);
    setNote(null);
    current.undo().catch(() => {
      pin({
        title: tr("That didn't come back.", '这次没撤回来。'),
        body: current.failure,
        undoable: false,
        mood: 'confused',
      });
    });
  }, [pin, tr]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.shiftKey) return;
      if (event.key.toLowerCase() !== 'z' || latest.current === null) return;
      // Inside a field ⌘Z belongs to the text.
      if (editsText(event.target)) return;
      event.preventDefault();
      undoLast();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [undoLast]);

  return (
    <UndoContext.Provider value={{ offer, say, undoLast, canUndo: last !== null }}>
      {children}
      {note !== null && (
        <div className="tpi tpi-undo">
          <Note label={note.title}>
            <div className="tpi-undo-head">
              {note.mood !== null && <Mascot state={note.mood} size={26} />}
              <p className="tpi-note-title">{note.title}</p>
            </div>
            <p className="tpi-note-body">{note.body}</p>
            <div className="tpi-note-actions">
              {note.undoable && (
                <button type="button" className="tpi-note-yes" onClick={undoLast}>
                  {tr('Undo ⌘Z', '撤回 ⌘Z')}
                </button>
              )}
              <button type="button" className="tpi-note-no" onClick={() => setNote(null)}>
                {tr('Got it', '知道了')}
              </button>
            </div>
          </Note>
        </div>
      )}
    </UndoContext.Provider>
  );
}

export function useUndo(): UndoContextValue {
  return useContext(UndoContext);
}

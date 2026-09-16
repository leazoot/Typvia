// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useEffect, useRef } from 'react';
import { useLocation, useNavigate } from 'react-router';

/**
 * What the menu bar can ask of the window. The menu only names the command;
 * whichever page is showing and can act on it does.
 */
export type AppCommand =
  | 'snippet.edit'
  | 'snippet.copy-plain'
  | 'snippet.to-vault'
  /** Payload: the collection's folder id, or '' for Unsorted. */
  | 'snippet.move'
  | 'snippet.delete'
  | 'collection.new'
  | 'vault.peek'
  | 'trash.restore-picked'
  | 'trash.purge-picked'
  | 'trash.empty';

type Handler = (payload: string) => void;

const handlers = new Map<AppCommand, Handler[]>();

/** Runs the most recently mounted handler; false when nothing on screen takes it. */
export function runCommand(command: AppCommand, payload = ''): boolean {
  const list = handlers.get(command);
  const handler = list?.[list.length - 1];
  if (handler === undefined) return false;
  handler(payload);
  return true;
}

/** Router state that carries a command to the page a menu opened for it. */
export function commandState(command: AppCommand): { command: AppCommand } {
  return { command };
}

function arrivedWith(state: unknown): unknown {
  return typeof state === 'object' && state !== null && 'command' in state
    ? (state as { command: unknown }).command
    : null;
}

/**
 * Takes a menu command while the component is mounted. When the navigation
 * that mounted the page carried this command, it runs once on arrival.
 */
export function useCommand(command: AppCommand, handler: Handler): void {
  const latest = useRef(handler);
  latest.current = handler;
  const location = useLocation();
  const navigate = useNavigate();
  const arrived = arrivedWith(location.state) === command;

  useEffect(() => {
    const run: Handler = (payload) => latest.current(payload);
    handlers.set(command, [...(handlers.get(command) ?? []), run]);
    return () => {
      handlers.set(
        command,
        (handlers.get(command) ?? []).filter((entry) => entry !== run),
      );
    };
  }, [command]);

  useEffect(() => {
    if (!arrived) return;
    void navigate(location.pathname, { replace: true, state: null });
    latest.current('');
  }, [arrived, location.pathname, navigate]);
}

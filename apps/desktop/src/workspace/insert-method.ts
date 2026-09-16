// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { InjectionMethod } from '@typvia/shared';
import { useCallback, useEffect, useState } from 'react';

/**
 * How an insert reaches the front app. It is a device-local preference that
 * carries no snippet content, so it sits beside the language and theme
 * preferences and reaches the panel window through the `storage` event.
 */
export const INSERT_METHOD_KEY = 'tv.ui.insertMethod';

export function loadInsertMethod(): InjectionMethod {
  try {
    return localStorage.getItem(INSERT_METHOD_KEY) === 'keystrokes' ? 'keystrokes' : 'paste';
  } catch {
    return 'paste';
  }
}

export function useInsertMethod(): [InjectionMethod, (method: InjectionMethod) => void] {
  const [method, setMethod] = useState<InjectionMethod>(loadInsertMethod);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === INSERT_METHOD_KEY) setMethod(loadInsertMethod());
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, []);

  const choose = useCallback((next: InjectionMethod) => {
    try {
      localStorage.setItem(INSERT_METHOD_KEY, next);
    } catch {
      // Storage refused: the choice still holds for this window until it closes.
    }
    setMethod(next);
  }, []);

  return [method, choose];
}

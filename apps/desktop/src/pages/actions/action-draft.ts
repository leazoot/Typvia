// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type {
  AiAction,
  AiActionInputSource,
  AiActionOutputMode,
  AiActionPermissionScope,
  AiActionSaveInput,
} from '@typvia/shared';

/** An editable copy of one action; `id === null` is an unsaved new one. */
export interface ActionDraft {
  id: string | null;
  name: string;
  promptTemplate: string;
  providerId: string | null;
  model: string;
  inputSource: AiActionInputSource;
  outputMode: AiActionOutputMode;
  permissionScope: AiActionPermissionScope;
  /** Kept as entered; blank means the provider's default. */
  temperature: string;
  isBuiltin: boolean;
}

export function toDraft(action: AiAction): ActionDraft {
  return {
    id: action.id,
    name: action.name,
    promptTemplate: action.promptTemplate,
    providerId: action.providerId,
    model: action.model ?? '',
    inputSource: action.inputSource,
    outputMode: action.outputMode,
    permissionScope: action.permissionScope,
    temperature: action.temperature === null ? '' : String(action.temperature),
    isBuiltin: action.isBuiltin,
  };
}

export function saveInput(draft: ActionDraft): AiActionSaveInput {
  const temperature = draft.temperature.trim();
  return {
    id: draft.id,
    name: draft.name.trim(),
    promptTemplate: draft.promptTemplate,
    providerId: draft.providerId,
    model: draft.model.trim() === '' ? null : draft.model.trim(),
    inputSource: draft.inputSource,
    outputMode: draft.outputMode,
    permissionScope: draft.permissionScope,
    temperature: temperature === '' ? null : Number(temperature),
  };
}

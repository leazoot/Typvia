// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * AI IPC surface, mirroring the Rust DTOs in
 * crates/host-service/src/ai.rs and apps/desktop/src-tauri/src/ai/.
 * API keys travel one way only — into `aiApiKeySet` — and never appear in
 * any response shape.
 */
import { invoke } from '@tauri-apps/api/core';
import type { TemplateFieldType } from './client';
import { toIpcError } from './error';
import type { Folder, Tag } from './types';

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw toIpcError(raw);
  }
}

export interface AiProvider {
  id: string;
  name: string;
  kind: string;
  baseUrl: string;
  model: string;
  timeoutMs: number;
  hasApiKey: boolean;
}

export interface AiProviderSaveInput {
  /** Absent on create. */
  id?: string | null;
  name: string;
  kind: string;
  /** Blank means "use the kind's default local endpoint" (Ollama / LM Studio). */
  baseUrl: string;
  model: string;
  /** `0` means the default (30s). */
  timeoutMs: number;
}

export interface OrganizeDraftInput {
  title: string;
  body: string;
  description: string | null;
  isSensitive: boolean;
}

/** A validated suggestion; absent fields mean nothing usable was offered. */
export interface OrganizeSuggestion {
  title: string | null;
  description: string | null;
  snippetType: string | null;
  securityLevel: string | null;
  trigger: string | null;
  tags: Tag[];
  folder: Folder | null;
}

export interface ExtractDraftInput {
  body: string;
  isSensitive: boolean;
}

/** One extraction proposal: replace `original` with `{{name}}` and add a
 *  field. Every proposal returned has already been validated as applicable
 *  (span present, name legal and collision-free). */
export interface VariableProposal {
  name: string;
  original: string;
  /** Always from the proposable subset (choice types and secret_ref are
   *  never proposed); the Rust validator guarantees it. */
  fieldType: TemplateFieldType;
  defaultValue: string | null;
}

export interface AiConnectivity {
  /** `null`: reachable, but the endpoint does not enumerate models. */
  modelAvailable: boolean | null;
}

export type AiActionInputSource = 'selection' | 'clipboard' | 'snippet' | 'share';
export type AiActionOutputMode = 'replace' | 'insert' | 'copy' | 'new_snippet';
export type AiActionPermissionScope = 'normal_only' | 'mask_secrets';

/** One AI action definition. */
export interface AiAction {
  id: string;
  name: string;
  promptTemplate: string;
  /** `null`: unconfigured — running reports "no provider configured". */
  providerId: string | null;
  /** `null`: use the provider's default model. */
  model: string | null;
  inputSource: AiActionInputSource;
  outputMode: AiActionOutputMode;
  permissionScope: AiActionPermissionScope;
  temperature: number | null;
  /** Seeded built-in; editable like any action, the flag only labels it. */
  isBuiltin: boolean;
}

export interface AiActionSaveInput {
  /** Absent on create. */
  id?: string | null;
  name: string;
  promptTemplate: string;
  providerId: string | null;
  model: string | null;
  inputSource: AiActionInputSource;
  outputMode: AiActionOutputMode;
  permissionScope: AiActionPermissionScope;
  temperature: number | null;
}

/** The text one action run operates on. `source` must match the action's
 *  configured input source (the action is a contract, not a hint). */
export interface ActionRunInput {
  text: string;
  source: AiActionInputSource;
  isSensitive: boolean;
}

/** A finished run, pending the user's confirmation. `maskedKinds` lists
 *  the detector kinds stripped before sending (`mask_secrets` scope). */
export interface AiActionRunResult {
  output: string;
  maskedKinds: string[];
}

export function aiProviderList(): Promise<AiProvider[]> {
  return call('ai_provider_list');
}

export function aiProviderSave(input: AiProviderSaveInput): Promise<AiProvider> {
  return call('ai_provider_save', { input });
}

export function aiProviderDelete(id: string): Promise<void> {
  return call('ai_provider_delete', { id });
}

export function aiApiKeySet(providerId: string, key: string): Promise<void> {
  return call('ai_api_key_set', { providerId, key });
}

export function aiApiKeyClear(providerId: string): Promise<void> {
  return call('ai_api_key_clear', { providerId });
}

export function aiCheckConnectivity(providerId: string): Promise<AiConnectivity> {
  return call('ai_check_connectivity', { providerId });
}

export function aiExtractVariables(
  providerId: string,
  input: ExtractDraftInput,
): Promise<VariableProposal[]> {
  return call('ai_extract_variables', { providerId, input });
}

export function aiOrganize(
  providerId: string,
  input: OrganizeDraftInput,
): Promise<OrganizeSuggestion> {
  return call('ai_organize', { providerId, input });
}

export function aiActionList(): Promise<AiAction[]> {
  return call('ai_action_list');
}

export function aiActionSave(input: AiActionSaveInput): Promise<AiAction> {
  return call('ai_action_save', { input });
}

export function aiActionDelete(id: string): Promise<void> {
  return call('ai_action_delete', { id });
}

export function aiActionRun(actionId: string, input: ActionRunInput): Promise<AiActionRunResult> {
  return call('ai_action_run', { actionId, input });
}

/** One row of the un-disableable egress receipt: metadata only — the table
 *  it reads from structurally cannot hold prompt or response content. */
export interface AiEgressEntry {
  id: number;
  occurredAt: number;
  providerId: string;
  requestClass: string;
  requestBytes: number;
}

export interface AiEgressPage {
  /** Newest first. */
  entries: AiEgressEntry[];
  /** All-time total, so the viewer can state a true count while paging. */
  total: number;
}

export function aiEgressLogList(limit: number, offset: number): Promise<AiEgressPage> {
  return call('ai_egress_log_list', { limit, offset });
}

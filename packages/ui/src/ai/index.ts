/**
 * Shared AI UI layer. Desktop and mobile draw AI differently —
 * a two-pane actions route vs. editor strips and pushed settings pages —
 * but the flows, the state machines and the vocabulary are identical, so
 * they exist once here and each host only contributes its layout.
 *
 * Like `@typvia/ui/sync`, this module reaches the typed IPC client and is
 * exported under `@typvia/ui/ai` so the purely presentational components
 * stay free of it.
 */
export { confirmLabel, countWords, modeLabel, scopeLabel, sourceLabel } from './action-labels';
export { egressClassLabel, egressEntryTime, egressTodayCount } from './egress';
export { suggestionFields } from './organize-fields';
export type { OrganizeCurrent, OrganizeFieldOffer } from './organize-fields';
export { AI_PROVIDER_PREF, chooseProvider, rememberProvider } from './provider-choice';
export { OLLAMA_KIND, PROVIDER_KINDS, kindLabel } from './provider-kinds';
export type { KindOption } from './provider-kinds';
export { useActionRun } from './use-action-run';
export type { ActionRun } from './use-action-run';
export { useOrganize } from './use-organize';
export type { OrganizeDraftFields, OrganizePhase } from './use-organize';

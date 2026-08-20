import type { AiProvider } from '@typvia/shared';

/**
 * Which provider an AI control targets. The remembered id is a UI
 * preference only (never content); the authoritative choice is still the
 * explicit `providerId` each command call carries.
 */
export const AI_PROVIDER_PREF = 'tv.ai.provider';

/** Picks `wantedId` when it exists, else the remembered one, else the
 *  first by list order; `null` means nothing is configured. */
export function chooseProvider(providers: AiProvider[], wantedId?: string): AiProvider | null {
  if (providers.length === 0) return null;
  const remembered = localStorage.getItem(AI_PROVIDER_PREF);
  return (
    providers.find((p) => p.id === wantedId) ??
    providers.find((p) => p.id === remembered) ??
    providers[0] ??
    null
  );
}

export function rememberProvider(id: string): void {
  localStorage.setItem(AI_PROVIDER_PREF, id);
}

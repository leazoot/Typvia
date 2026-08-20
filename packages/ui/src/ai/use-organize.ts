import {
  aiOrganize,
  type AiProvider,
  aiProviderList,
  ipcErrorCopy,
  type OrganizeSuggestion,
} from '@typvia/shared';
import { useState } from 'react';
import { useTr } from '../i18n';
import { chooseProvider, rememberProvider } from './provider-choice';

/**
 * Save-time organizing flow, shared by the desktop editor
 * strip and the mobile editor. Nothing here writes: the suggestion is held
 * until the host applies confirmed fields through its normal save path.
 */
export type OrganizePhase =
  | { at: 'idle' }
  | { at: 'loading' }
  | { at: 'unconfigured' }
  | { at: 'error'; message: string }
  | { at: 'ready'; suggestion: OrganizeSuggestion; providers: AiProvider[]; providerId: string };

export interface OrganizeDraftFields {
  title: string;
  body: string;
  description: string | null;
}

export function useOrganize() {
  const tr = useTr();
  const [phase, setPhase] = useState<OrganizePhase>({ at: 'idle' });

  const run = async (draft: OrganizeDraftFields, providerId?: string): Promise<void> => {
    setPhase({ at: 'loading' });
    try {
      const providers = await aiProviderList();
      const provider = chooseProvider(providers, providerId);
      if (provider === null) {
        setPhase({ at: 'unconfigured' });
        return;
      }
      rememberProvider(provider.id);
      const suggestion = await aiOrganize(provider.id, {
        title: draft.title,
        body: draft.body,
        description: draft.description,
        // Both editors only ever hold normal drafts; sensitive content is
        // authored through the vault flow and never reaches here.
        isSensitive: false,
      });
      setPhase({ at: 'ready', suggestion, providers, providerId: provider.id });
    } catch (caught) {
      const message =
        caught instanceof Error
          ? tr(...ipcErrorCopy(caught))
          : tr('the suggestion failed', '建议生成失败');
      setPhase({ at: 'error', message });
    }
  };

  const reset = () => setPhase({ at: 'idle' });

  return { phase, run, reset };
}

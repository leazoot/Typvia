import type { OrganizeSuggestion } from '@typvia/shared';

/** The editor fields the suggestion is compared against. */
export interface OrganizeCurrent {
  title: string;
  description: string | null;
  snippetType: string;
  trigger: string | null;
  triggerMode: string | null;
  folderId: string | null;
}

/** One applicable suggestion: what to show and what applying it changes. */
export interface OrganizeFieldOffer {
  key: 'title' | 'description' | 'type' | 'trigger' | 'folder';
  label: string;
  labelZh: string;
  value: string;
  changes: {
    title?: string;
    description?: string;
    snippetType?: string;
    trigger?: string;
    triggerMode?: string;
    folderId?: string;
  };
}

/**
 * Keeps only suggestions that would change something. A suggested trigger
 * is offered only when the draft has none — organizing never overwrites a
 * trigger the user already chose.
 */
export function suggestionFields(
  suggestion: OrganizeSuggestion,
  current: OrganizeCurrent,
): OrganizeFieldOffer[] {
  const fields: OrganizeFieldOffer[] = [];
  if (suggestion.title !== null && suggestion.title !== current.title) {
    fields.push({
      key: 'title',
      label: 'Title',
      labelZh: '标题',
      value: suggestion.title,
      changes: { title: suggestion.title },
    });
  }
  if (suggestion.description !== null && suggestion.description !== current.description) {
    fields.push({
      key: 'description',
      label: 'Description',
      labelZh: '描述',
      value: suggestion.description,
      changes: { description: suggestion.description },
    });
  }
  if (suggestion.snippetType !== null && suggestion.snippetType !== current.snippetType) {
    fields.push({
      key: 'type',
      label: 'Type',
      labelZh: '类型',
      value: suggestion.snippetType,
      changes: { snippetType: suggestion.snippetType },
    });
  }
  if (suggestion.trigger !== null && current.trigger === null) {
    fields.push({
      key: 'trigger',
      label: 'Trigger',
      labelZh: '触发词',
      value: suggestion.trigger,
      changes: { trigger: suggestion.trigger, triggerMode: current.triggerMode ?? 'delimiter' },
    });
  }
  if (suggestion.folder !== null && suggestion.folder.id !== current.folderId) {
    fields.push({
      key: 'folder',
      label: 'Folder',
      labelZh: '文件夹',
      value: suggestion.folder.name,
      changes: { folderId: suggestion.folder.id },
    });
  }
  return fields;
}

/**
 * Editor AI strip on mobile: save-time organizing and actions over the
 * draft content, in the create sheet's quiet language — text controls, no
 * modal, no spinner, no icon. The flows and vocabulary are the shared
 * `@typvia/ui/ai` layer; only this layout is mobile's.
 *
 * With AI off (no provider configured on this phone) the whole strip is
 * hidden, not greyed. Nothing here writes until the user applies a
 * suggestion or confirms a run result.
 */
import {
  aiActionList,
  aiProviderList,
  batchTagSnippets,
  createSnippet,
  type AiAction,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { confirmLabel, suggestionFields, useActionRun, useOrganize } from '@typvia/ui/ai';
import { useEffect, useState } from 'react';

export interface AiStripApply {
  title?: string;
  snippetType?: string;
  trigger?: string;
  folderId?: string;
}

interface AiStripProps {
  /** Present when editing; suggestions can then also apply tags. */
  snippetId: string | null;
  title: string;
  body: string;
  snippetType: string;
  folderId: string | null;
  /** As typed; blank means none. */
  trigger: string;
  description: string | null;
  onApply: (changes: AiStripApply) => void;
  onBodyChange: (body: string) => void;
}

export function AiStrip(props: AiStripProps) {
  const tr = useTr();
  const { snippetId, title, body, snippetType, folderId, trigger, description } = props;
  // `null` while loading; AI off resolves to an empty provider list.
  const [actions, setActions] = useState<AiAction[] | null>(null);
  const [aiOn, setAiOn] = useState(false);
  const [actionId, setActionId] = useState('');
  const { phase, run: runOrganize, reset } = useOrganize();
  const { run, start, applied, failed, dismiss } = useActionRun();
  const [tagsApplied, setTagsApplied] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.all([aiProviderList(), aiActionList()])
      .then(([providers, loaded]) => {
        if (cancelled) return;
        setAiOn(providers.length > 0);
        setActions(loaded);
        setActionId((current) => (current === '' ? (loaded[0]?.id ?? '') : current));
      })
      .catch(() => {
        // Unreadable AI state reads as AI off; the editor is unaffected.
        if (!cancelled) setAiOn(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!aiOn || actions === null) return null;

  const organize = () => {
    setTagsApplied(false);
    void runOrganize({ title, body, description });
  };

  const action = actions.find((a) => a.id === actionId) ?? null;

  const runAction = () => {
    if (action === null || body.trim() === '') return;
    void start(action.id, {
      text: body,
      source: action.inputSource,
      // The editor only ever holds normal drafts; sensitive content is
      // authored through the vault flow and never reaches here.
      isSensitive: false,
    });
  };

  const confirm = async (output: string) => {
    if (action === null) return;
    switch (action.outputMode) {
      case 'replace':
        props.onBodyChange(output);
        applied(tr('Content replaced.', '内容已替换。'));
        return;
      case 'insert':
        props.onBodyChange(`${body}\n\n${output}`);
        applied(tr('Inserted below the content.', '已插入到内容下方。'));
        return;
      case 'copy':
        try {
          await navigator.clipboard.writeText(output);
          applied(tr('Copied to the clipboard.', '已复制到剪贴板。'));
        } catch {
          failed(
            tr(
              'the clipboard refused the copy — the result is above',
              '剪贴板拒绝了此次复制 —— 结果仍显示在上方',
            ),
          );
        }
        return;
      case 'new_snippet': {
        try {
          await createSnippet({ title: action.name, body: output, snippetType: 'text' });
          applied(tr('Saved to your library as a snippet.', '已作为片段保存到你的片段库。'));
        } catch {
          failed(
            tr(
              'the snippet could not be saved — the result is above',
              '片段未能保存 —— 结果仍显示在上方',
            ),
          );
        }
        return;
      }
    }
  };

  // The mobile editor has no description field, so a description suggestion
  // would be an offer it cannot hold — filtered rather than greyed.
  const fields =
    phase.at === 'ready'
      ? suggestionFields(phase.suggestion, {
          title,
          description,
          snippetType,
          trigger: trigger.trim() === '' ? null : trigger.trim(),
          triggerMode: null,
          folderId,
        }).filter((field) => field.key !== 'description')
      : [];

  const applyTags = () => {
    if (phase.at !== 'ready' || snippetId === null) return;
    Promise.all(phase.suggestion.tags.map((tag) => batchTagSnippets([snippetId], tag.id)))
      .then(() => setTagsApplied(true))
      .catch(() => {
        // Tags are additive and retryable; the row stays offered.
      });
  };

  return (
    <section className="tv-med-ai" aria-label="AI">
      <div className="tv-med-ai-label">{tr('AI', 'AI 整理与动作')}</div>

      {phase.at === 'idle' && (
        <button
          type="button"
          className="tv-med-ai-run"
          disabled={body.trim() === ''}
          onClick={organize}
        >
          {tr('Organize with AI', 'AI 整理')}
        </button>
      )}
      {phase.at === 'loading' && (
        <p className="tv-med-ai-note" role="status">
          {tr('Organizing…', '整理中…')}
        </p>
      )}
      {phase.at === 'unconfigured' && (
        <p className="tv-med-ai-note" role="status">
          {tr(
            'Everything here works without AI — suggestions need a provider (Settings · AI).',
            '这里的一切无需 AI 也能用 —— 获取建议需要先配置服务商(设置 · AI)。',
          )}
        </p>
      )}
      {phase.at === 'error' && (
        <div className="tv-med-ai-block" role="status">
          <p className="tv-med-ai-note">
            {tr(`Your draft is safe — ${phase.message}`, `你的草稿完好无损 —— ${phase.message}`)}
          </p>
          <div className="tv-med-ai-bar">
            <button type="button" className="tv-med-ai-run" onClick={organize}>
              {tr('Try again', '重试')}
            </button>
            <button type="button" className="tv-med-ai-quiet" onClick={reset}>
              {tr('Dismiss', '关闭')}
            </button>
          </div>
        </div>
      )}
      {phase.at === 'ready' && (
        <div className="tv-med-ai-block" role="group" aria-label={tr('AI suggestions', 'AI 建议')}>
          {fields.length === 0 && phase.suggestion.tags.length === 0 ? (
            <p className="tv-med-ai-note" role="status">
              {tr(
                'No suggestions — this draft already looks organized.',
                '没有建议 —— 这份草稿看起来已经很有条理。',
              )}
            </p>
          ) : (
            <>
              <p className="tv-med-ai-note" role="status">
                {tr('Suggestions — apply what fits:', '以下是建议 —— 按需应用:')}
              </p>
              {fields.map((field) => (
                <button
                  key={field.key}
                  type="button"
                  className="tv-med-ai-field"
                  onClick={() => props.onApply(field.changes)}
                >
                  <span className="tv-med-ai-field-label">{tr(field.label, field.labelZh)}</span>
                  <span className="tv-med-ai-field-value">{field.value}</span>
                </button>
              ))}
              {phase.suggestion.tags.length > 0 && snippetId !== null && (
                <button
                  type="button"
                  className="tv-med-ai-field"
                  disabled={tagsApplied}
                  onClick={applyTags}
                >
                  <span className="tv-med-ai-field-label">{tr('Tags', '标签')}</span>
                  <span className="tv-med-ai-field-value">
                    {tagsApplied
                      ? tr('Applied', '已应用')
                      : phase.suggestion.tags.map((tag) => tag.name).join(', ')}
                  </span>
                </button>
              )}
            </>
          )}
          {phase.suggestion.securityLevel === 'sensitive' && (
            <p className="tv-med-ai-note">
              {tr(
                'Looks sensitive — consider the Vault, which encrypts it.',
                '内容看起来较敏感 —— 可以考虑放入保险库,它会加密保存。',
              )}
            </p>
          )}
          <button type="button" className="tv-med-ai-quiet" onClick={reset}>
            {tr('Dismiss', '关闭')}
          </button>
        </div>
      )}

      {actions.length > 0 && (
        <div className="tv-med-ai-actions">
          <select
            className="tv-med-select"
            aria-label={tr('AI action', 'AI 动作')}
            value={actionId}
            onChange={(e) => {
              setActionId(e.target.value);
              dismiss();
            }}
          >
            {actions.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="tv-med-ai-run"
            disabled={action === null || body.trim() === '' || run.at === 'running'}
            onClick={runAction}
          >
            {tr('Run on content', '对内容运行')}
          </button>
        </div>
      )}

      {run.at === 'running' && (
        <p className="tv-med-ai-note" role="status" aria-busy="true">
          {tr('Running…', '运行中…')}
        </p>
      )}
      {run.at === 'error' && (
        <div className="tv-med-ai-block" role="status">
          <p className="tv-med-ai-note">
            {tr(`Your content is unchanged — ${run.message}`, `你的内容没有改变 —— ${run.message}`)}
          </p>
          <div className="tv-med-ai-bar">
            <button type="button" className="tv-med-ai-run" onClick={runAction}>
              {tr('Try again', '重试')}
            </button>
            <button type="button" className="tv-med-ai-quiet" onClick={dismiss}>
              {tr('Dismiss', '关闭')}
            </button>
          </div>
        </div>
      )}
      {run.at === 'pending' && action !== null && (
        <div className="tv-med-ai-block">
          <p className="tv-med-ai-note">
            {tr('Result · nothing applied until you confirm', '生成结果 · 确认前不会应用任何更改')}
          </p>
          {run.maskedKinds.length > 0 && (
            <p className="tv-med-ai-note" role="status">
              {tr(
                `Secrets stripped before sending · ${run.maskedKinds.join(', ')}`,
                `发送前已剥离疑似密钥 · ${run.maskedKinds.join(', ')}`,
              )}
            </p>
          )}
          <pre className="tv-med-ai-output">{run.output}</pre>
          <div className="tv-med-ai-bar">
            <button
              type="button"
              className="tv-med-ai-run"
              onClick={() => void confirm(run.output)}
            >
              {confirmLabel(action.outputMode, tr)}
            </button>
            <button type="button" className="tv-med-ai-quiet" onClick={runAction}>
              {tr('Run again', '再次运行')}
            </button>
            <button type="button" className="tv-med-ai-quiet" onClick={dismiss}>
              {tr('Dismiss', '关闭')}
            </button>
          </div>
          <p className="tv-med-ai-footnote">
            {tr(
              `${run.words} words left this phone · 1 request · ${run.seconds}s`,
              `${run.words} 个词离开了本机 · 1 次请求 · ${run.seconds} 秒`,
            )}
          </p>
        </div>
      )}
      {run.at === 'applied' && (
        <p className="tv-med-ai-note" role="status" aria-live="assertive">
          {run.note}
        </p>
      )}
    </section>
  );
}

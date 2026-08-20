import {
  copySnippet,
  listFolderChildren,
  syncDevices,
  syncStatus,
  trashSnippet,
  updateSnippet,
} from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { Toast, designTokens, useLocale, useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { agoPhrase, previewIsMono, typeWord } from '../../components/format';
import { pushBackHandler } from '../../platform';
import { TemplateFillSheet } from './template-fill-sheet';
import './detail.css';

interface DetailPageProps {
  snippet: Snippet;
  onBack: () => void;
  onEdit: (snippet: Snippet) => void;
  /** After a successful trash — the host closes the screen and refreshes. */
  onDeleted: (snippet: Snippet) => void;
}

/**
 * Snippet detail: title over a caps meta
 * line, the body in a tinted block, hairline info rows, a full-width ink
 * Copy button with quiet text actions under it, and the trigger hint on the
 * caret. Locked sensitive rows never navigate here.
 */
export function DetailPage({ snippet, onBack, onEdit, onDeleted }: DetailPageProps) {
  const tr = useTr();
  const locale = useLocale();
  const [current, setCurrent] = useState(snippet);
  const [folderName, setFolderName] = useState<string | null>(null);
  const [deviceCount, setDeviceCount] = useState<number | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [filling, setFilling] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Android system back = the ← affordance.
  useEffect(() => pushBackHandler(onBack), [onBack]);

  useEffect(
    () => () => {
      if (toastTimer.current !== null) clearTimeout(toastTimer.current);
    },
    [],
  );

  // Folder name for the meta line; a failed read just leaves it out.
  useEffect(() => {
    if (current.folderId === null) return;
    let cancelled = false;
    listFolderChildren(null)
      .then((roots) => {
        if (cancelled) return;
        const folder = roots.find((f) => f.id === current.folderId);
        setFolderName(folder === undefined ? null : folder.name);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [current.folderId]);

  // The Synced row appears only when sync is really on (honest facts only).
  useEffect(() => {
    let cancelled = false;
    syncStatus()
      .then((status) => {
        if (cancelled || !status.enabled) return;
        return syncDevices().then((devices) => {
          if (!cancelled) {
            setDeviceCount(devices.filter((d) => d.revokedAt === null).length);
          }
        });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const showToast = (message: string) => {
    setToast(message);
    if (toastTimer.current !== null) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => {
      setToast(null);
    }, designTokens.motion.duration.toastLife);
  };

  const copy = () => {
    // A template's Copy goes through the fill sheet.
    if (current.snippetType === 'template') {
      setFilling(true);
      return;
    }
    copySnippet(current.id)
      .then(() => {
        showToast(tr('Copied', '已复制'));
      })
      .catch(() => {
        showToast(tr('Copy failed — your snippet is unchanged.', '复制失败——片段本身完好。'));
      });
  };

  const toggleFavorite = () => {
    if (current.body === null) return;
    updateSnippet({
      id: current.id,
      title: current.title,
      body: current.body,
      snippetType: current.snippetType,
      description: current.description,
      folderId: current.folderId,
      trigger: current.trigger,
      triggerMode: current.triggerMode,
      language: current.language,
      isFavorite: !current.isFavorite,
      isPinned: current.isPinned,
      isEnabled: current.isEnabled,
    })
      .then((updated) => {
        setCurrent(updated);
      })
      .catch(() => {
        showToast(tr('Change failed — nothing was saved.', '修改失败——没有任何改动被保存。'));
      });
  };

  const remove = () => {
    trashSnippet(current.id)
      .then(() => {
        onDeleted(current);
      })
      .catch(() => {
        setConfirmingDelete(false);
        showToast(tr('Delete failed — the snippet is still here.', '删除失败——片段还在。'));
      });
  };

  const metaParts = [typeWord(current.snippetType, locale)];
  if (folderName !== null) metaParts.push(folderName);
  if (current.usageCount > 0) {
    metaParts.push(tr(`${current.usageCount} uses`, `${current.usageCount} 次使用`));
  }

  return (
    <main className="tv-mobile-page tv-mdetail">
      <div className="tv-mdetail-top">
        <button
          type="button"
          className="tv-mdetail-back"
          aria-label={tr('Back', '返回')}
          onClick={onBack}
        >
          ←
        </button>
      </div>

      <header className="tv-mdetail-head">
        <h1 className="tv-mdetail-title">{current.title}</h1>
        <div className="tv-mdetail-meta">{metaParts.join(' · ')}</div>
      </header>

      <div
        className={
          previewIsMono(current.snippetType) ? 'tv-mdetail-body is-mono' : 'tv-mdetail-body'
        }
      >
        {current.body}
      </div>

      <div className="tv-mdetail-rows">
        {current.trigger !== null && current.trigger !== '' && (
          <div className="tv-mdetail-row">
            <span className="tv-mdetail-row-label">{tr('Trigger', '触发词')}</span>
            <span className="tv-mdetail-row-value is-mono">{current.trigger}</span>
          </div>
        )}
        {folderName !== null && (
          <div className="tv-mdetail-row">
            <span className="tv-mdetail-row-label">{tr('Folder', '文件夹')}</span>
            <span className="tv-mdetail-row-value">{folderName}</span>
          </div>
        )}
        {current.lastUsedAt !== null && (
          <div className="tv-mdetail-row">
            <span className="tv-mdetail-row-label">{tr('Last used', '最近使用')}</span>
            <span className="tv-mdetail-row-value">
              {agoPhrase(current.lastUsedAt, Date.now(), locale)}
            </span>
          </div>
        )}
        {deviceCount !== null && (
          <div className="tv-mdetail-row">
            <span className="tv-mdetail-row-label">{tr('Synced', '已同步')}</span>
            <span className="tv-mdetail-row-value">
              {tr(`${deviceCount} device${deviceCount === 1 ? '' : 's'}`, `${deviceCount} 台设备`)}
            </span>
          </div>
        )}
      </div>

      <div className="tv-mdetail-actions">
        <button type="button" className="tv-mdetail-copy" onClick={copy}>
          {tr('Copy', '复制')}
        </button>
        <div className="tv-mdetail-quiet-actions">
          {confirmingDelete ? (
            <>
              <span className="tv-mdetail-confirm">
                {tr('Delete 1 snippet?', '删除这 1 条片段？')}
              </span>
              <button type="button" className="tv-mdetail-quiet" onClick={remove}>
                {tr('Delete', '删除')}
              </button>
              <button
                type="button"
                className="tv-mdetail-quiet"
                onClick={() => {
                  setConfirmingDelete(false);
                }}
              >
                {tr('Keep', '保留')}
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                className="tv-mdetail-quiet"
                onClick={() => {
                  onEdit(current);
                }}
              >
                {tr('Edit', '编辑')}
              </button>
              <button type="button" className="tv-mdetail-quiet" onClick={toggleFavorite}>
                {current.isFavorite ? tr('Favorited', '已收藏') : tr('Favorite', '收藏')}
              </button>
              <button
                type="button"
                className="tv-mdetail-quiet"
                onClick={() => {
                  setConfirmingDelete(true);
                }}
              >
                {tr('Delete', '删除')}
              </button>
            </>
          )}
        </div>
        {current.trigger !== null && current.trigger !== '' && (
          <div className="tv-mdetail-hint">
            <span className="tv-mdetail-hint-bar" />
            <span>
              {tr('Type ', '在任何地方输入 ')}
              <span className="tv-mdetail-hint-trigger">{current.trigger}</span>
              {tr(' anywhere to insert', ' 即可插入')}
            </span>
          </div>
        )}
      </div>

      {filling && (
        <TemplateFillSheet
          snippet={current}
          onClose={() => {
            setFilling(false);
          }}
          onCopied={() => {
            setFilling(false);
            showToast(tr('Copied', '已复制'));
          }}
        />
      )}

      {toast !== null && (
        <div className="tv-mobile-toast">
          <Toast message={toast} assertive />
        </div>
      )}
    </main>
  );
}

/**
 * Settings · Privacy: claims stated in the negative, each verifiable, then
 * the "What left this phone" receipt. The egress log under "AI requests" is
 * written locally and cannot be turned off — a privacy claim you can't audit
 * is just a slogan. The log holds metadata only: the table it
 * reads from structurally cannot hold prompt or response content.
 */
import { aiEgressLogList, type AiEgressEntry } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { egressClassLabel, egressEntryTime, egressTodayCount } from '@typvia/ui/ai';
import { useEffect, useState } from 'react';
import { SettingsGroup, SettingsRow } from './rows';
import { ScreenSkeleton, SettingsScreen } from './screen';

const PAGE = 50;

interface PrivacyScreenProps {
  onBack: () => void;
  onEgressLog: () => void;
}

export function MobilePrivacyScreen({ onBack, onEgressLog }: PrivacyScreenProps) {
  const tr = useTr();
  const [page, setPage] = useState<{ entries: AiEgressEntry[]; total: number } | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    aiEgressLogList(PAGE, 0)
      .then((loaded) => {
        if (!cancelled) setPage(loaded);
      })
      .catch(() => {
        if (!cancelled) setLoadFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (page === null && !loadFailed) {
    return <ScreenSkeleton parent={tr('Settings', '设置')} onBack={onBack} />;
  }

  const aiValue = loadFailed
    ? tr('unreadable right now', '暂时读不出来')
    : page !== null
      ? tr(
          `${egressTodayCount(page.entries)} today · ${page.total} all time`,
          `今天 ${egressTodayCount(page.entries)} 条 · 累计 ${page.total} 条`,
        )
      : '';

  return (
    <SettingsScreen parent={tr('Settings', '设置')} title={tr('Privacy', '隐私')} onBack={onBack}>
      <SettingsGroup>
        <SettingsRow
          label={tr('No analytics, no crash reporting', '没有分析统计,没有崩溃上报')}
          sub={tr(
            'Not “anonymised” — absent. There is no analytics library in the build.',
            '不是「匿名化」—— 而是不存在。构建里没有任何分析库。',
          )}
        />
        <SettingsRow
          label={tr('The keyboard records nothing', '键盘不记录任何内容')}
          sub={tr(
            'It has no logging code and no network access at all when Full Access is off.',
            '它没有任何记录代码;未开启完全访问时,也完全没有网络访问。',
          )}
        />
        <SettingsRow
          label={tr('Vault items never reach AI', '保险库内容从不发送给 AI')}
          sub={tr(
            'Enforced before the request is built, not by asking the model to behave.',
            '在请求构建之前就被拦下,而不是靠请求模型自觉。',
          )}
        />
      </SettingsGroup>

      <SettingsGroup label={tr('What left this phone', '出网记录')}>
        <SettingsRow
          label={tr('Sync', '同步')}
          sub={tr(
            'Encrypted blobs and metadata only — never plaintext.',
            '只有加密数据块和元数据 —— 从不含明文。',
          )}
          value={tr('encrypted blobs', '加密数据块')}
        />
        <SettingsRow label={tr('AI requests', 'AI 请求')} value={aiValue} onPress={onEgressLog} />
        <SettingsRow
          label={tr('Anything else', '其他任何数据')}
          value={tr('Nothing. Ever.', '没有。永远没有。')}
        />
      </SettingsGroup>
      <p className="tv-mset-note">
        {tr(
          'This log is written locally and can’t be turned off — a privacy claim you can’t audit is just a slogan.',
          '出网日志在本地写入,不可关闭 —— 无法自行核验的隐私承诺只是口号。',
        )}
      </p>
    </SettingsScreen>
  );
}

interface EgressScreenProps {
  onBack: () => void;
}

export function MobileEgressLogScreen({ onBack }: EgressScreenProps) {
  const tr = useTr();
  const locale = useLocale();
  const [entries, setEntries] = useState<AiEgressEntry[]>([]);
  const [total, setTotal] = useState<number | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);

  const loadMore = (offset: number) => {
    aiEgressLogList(PAGE, offset)
      .then((loaded) => {
        setEntries((current) => (offset === 0 ? loaded.entries : [...current, ...loaded.entries]));
        setTotal(loaded.total);
      })
      .catch(() => {
        setLoadFailed(true);
      });
  };

  useEffect(() => {
    loadMore(0);
    // Initial load only; more pages are explicit.
  }, []);

  if (total === null && !loadFailed) {
    return <ScreenSkeleton parent={tr('Privacy', '隐私')} onBack={onBack} />;
  }

  return (
    <SettingsScreen
      parent={tr('Privacy', '隐私')}
      title={tr('AI requests', 'AI 请求')}
      intro={tr(
        'Every AI request this phone ever sent: when, to which provider, how many bytes. What was said is not recorded anywhere.',
        '这部手机发出过的每一次 AI 请求:何时、发给哪个提供方、多少字节。说了什么,任何地方都没有记录。',
      )}
      onBack={onBack}
    >
      {loadFailed ? (
        <p className="tv-mset-note">
          {tr(
            'The log itself is intact — it just could not be read. Leave and reopen to try again.',
            '日志本身完好 —— 只是暂时读不出来。退出后重新打开再试。',
          )}
        </p>
      ) : entries.length === 0 ? (
        <p className="tv-mset-note">
          {tr(
            'Nothing yet — no AI request has ever left this phone.',
            '还没有 —— 从未有 AI 请求离开这部手机。',
          )}
        </p>
      ) : (
        <>
          <SettingsGroup label={tr(`${total} total`, `共 ${total} 条`)}>
            {entries.map((entry) => (
              <SettingsRow
                key={entry.id}
                label={egressClassLabel(entry.requestClass, tr)}
                sub={entry.providerId}
                value={`${egressEntryTime(entry.occurredAt, locale)} · ${entry.requestBytes} B`}
              />
            ))}
          </SettingsGroup>
          {total !== null && entries.length < total && (
            <button
              type="button"
              className="tv-mset-button"
              onClick={() => loadMore(entries.length)}
            >
              {tr('Show older entries', '显示更早的记录')}
            </button>
          )}
        </>
      )}
    </SettingsScreen>
  );
}

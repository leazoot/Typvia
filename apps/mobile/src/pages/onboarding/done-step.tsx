// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { mobileBootstrap } from '@typvia/shared';
import type { MobileBootstrap } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { deviceWord, isAndroid } from '../../platform';

/** Chinese reading of the platform device word ('phone' / 'iPhone'). */
const deviceWordZh = isAndroid ? '手机' : 'iPhone';

interface DoneStepProps {
  /** Persists the marker and hands over to the shell. */
  onFinish: () => void;
}

/**
 * Final step — that's the whole product. Every status line is real: the
 * snippet count comes from the device DB, the keyboard line never claims
 * enablement the app has not verified (iOS offers no query API; on Android
 * this screen simply does not query the system list), and the Vault row is
 * an honest "Later" with the platform's own unlock word.
 */
export function DoneStep({ onFinish }: DoneStepProps) {
  const tr = useTr();
  // null = count still loading; a failed read falls back to honest copy.
  const [info, setInfo] = useState<MobileBootstrap | null>(null);
  const [countFailed, setCountFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    mobileBootstrap()
      .then((result) => {
        if (!cancelled) setInfo(result);
      })
      .catch(() => {
        if (!cancelled) setCountFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const total = info?.snippetTotal;
  const countTitle =
    total === undefined
      ? countFailed
        ? tr('Your library is ready', '你的片段库已就绪')
        : tr('Counting your snippets…', '正在统计片段…')
      : total === 0
        ? tr('Your library is ready — nothing saved yet', '片段库已就绪 —— 还没有保存内容')
        : tr(
            `${String(total)} snippet${total === 1 ? '' : 's'} in your library`,
            `片段库里有 ${String(total)} 条片段`,
          );
  const countSub = countFailed
    ? tr(
        'The count could not be read — your snippets are unaffected.',
        '数量未能读取 —— 你的片段不受影响。',
      )
    : tr(
        `Saved in a local database on this ${deviceWord}. Nothing leaves it.`,
        `保存在这台${deviceWordZh}的本地数据库中。任何内容都不会离开它。`,
      );

  return (
    <>
      <section className="tv-monb-body">
        <div className="tv-monb-done-head">
          <span aria-hidden="true" className="tv-monb-done-bar" />
          <h1 className="tv-monb-title">{tr("That's the whole product.", '这就是完整的产品。')}</h1>
        </div>
        <ul className="tv-monb-rows" aria-label={tr('Where things stand', '当前状态')}>
          <li className="tv-monb-row">
            <span aria-hidden="true" className="tv-monb-dot is-ok" />
            <span className="tv-monb-row-text">
              <span className="tv-monb-row-title">{countTitle}</span>
              <span className="tv-monb-row-sub">{countSub}</span>
            </span>
          </li>
          <li className="tv-monb-row">
            {/* Hollow dot: the state is unknown, and a filled status color
                would claim knowledge this screen does not have (no iOS query
                API; on Android the system list is not queried from here). */}
            <span aria-hidden="true" className="tv-monb-dot is-unknown" />
            <span className="tv-monb-row-text">
              <span className="tv-monb-row-title">
                {isAndroid
                  ? tr('Typvia bar', 'Typvia 输入栏')
                  : tr('Typvia Keyboard', 'Typvia 键盘')}
              </span>
              {isAndroid ? (
                <span className="tv-monb-row-sub">
                  {tr(
                    "If you switched Typvia on under Languages & input it's ready — the keyboard icon in the navigation bar switches to it in any app. This screen doesn't check the system list.",
                    '如果你已在「语言和输入法」里开启 Typvia,它就绪了 —— 在任何应用里都能通过导航栏的键盘图标切换过去。这个界面不会检查系统列表。',
                  )}
                </span>
              ) : (
                <span className="tv-monb-row-sub">
                  {tr(
                    "If you added the keyboard in Settings it's ready — try it in any app. iOS gives the app no way to check from here.",
                    '如果你已在设置里添加了键盘,它就绪了 —— 在任何应用里试试。iOS 不允许应用从这里检查。',
                  )}
                </span>
              )}
            </span>
          </li>
          <li className="tv-monb-row">
            <span aria-hidden="true" className="tv-monb-dot is-secure" />
            <span className="tv-monb-row-text">
              <span className="tv-monb-row-title">
                {tr('A place for keys and passwords', '存放密钥与密码的地方')}
              </span>
              <span className="tv-monb-row-sub">
                {tr(
                  `Masked, ${isAndroid ? 'your fingerprint' : 'Face ID'} on each use — arrives with the Vault update.`,
                  `始终掩码显示,每次使用需${isAndroid ? '指纹' : ' Face ID '}验证 —— 将随保险库更新到来。`,
                )}
              </span>
            </span>
            <span className="tv-monb-row-later">{tr('Later', '稍后')}</span>
          </li>
        </ul>
      </section>
      <footer className="tv-monb-foot">
        <button type="button" className="tv-monb-primary" onClick={onFinish}>
          {tr('Start using Typvia', '开始使用 Typvia')}
        </button>
        <span className="tv-monb-foot-note">
          {tr('Nothing else to set up.', '没有其他需要设置的了。')}
        </span>
      </footer>
    </>
  );
}

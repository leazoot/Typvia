// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { openKeyboardSettings } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useState } from 'react';
import { isAndroid } from '../../platform';

interface KeyboardStepProps {
  onContinue: () => void;
}

/**
 * Step 4 — get the keyboard working, in the platform's own terms. Both
 * branches share the shape (real settings jump, numbered path, honest
 * degradation, skip) and neither ever claims anything was enabled; only the
 * facts differ, so the fork lives here and not in the page composition.
 */
export function KeyboardStep({ onContinue }: KeyboardStepProps) {
  return isAndroid ? (
    <AndroidKeyboardStep onContinue={onContinue} />
  ) : (
    <IosKeyboardStep onContinue={onContinue} />
  );
}

/** Shared failure copy: what is still good first, then the manual path. */
function openFailedCopy(tr: Tr): string {
  return tr(
    'Settings did not open — nothing is lost. Follow the numbered path by hand; the app keeps working either way.',
    '设置未能打开 —— 没有任何损失。按上面编号的路径手动操作即可;无论如何应用都照常工作。',
  );
}

/**
 * iOS offers no programmatic enablement and no way to query it afterwards,
 * so this branch draws the real Settings path, offers the direct jump, and
 * never claims anything was enabled.
 */
function IosKeyboardStep({ onContinue }: KeyboardStepProps) {
  const tr = useTr();
  const [openFailed, setOpenFailed] = useState(false);

  const openSettings = () => {
    openKeyboardSettings().catch(() => {
      setOpenFailed(true);
    });
  };

  return (
    <>
      <section className="tv-monb-body">
        <h1 className="tv-monb-title">{tr('Add the keyboard.', '添加键盘。')}</h1>
        <ol className="tv-monb-path" aria-label={tr('Settings path', '设置路径')}>
          <li>{tr('Settings → General → Keyboard', '设置 → 通用 → 键盘')}</li>
          <li>{tr('Keyboards → Add New Keyboard', '键盘 → 添加新键盘')}</li>
          <li>{tr('Typvia Keyboard', 'Typvia 键盘')}</li>
        </ol>
        <p className="tv-monb-lead">
          {tr(
            'The button below opens Settings directly. Come back here once the keyboard is on the list — iOS gives the app no way to check for itself.',
            '下方按钮会直接打开设置。键盘出现在列表里之后再回到这里 —— iOS 不允许应用自行检查。',
          )}
        </p>
        <p className="tv-monb-lead">
          {tr(
            'Without the keyboard, nothing else is lost: Typvia still saves and copies snippets on this phone.',
            '没有键盘也不影响其他功能:Typvia 依然可以在这台手机上保存和复制片段。',
          )}
        </p>
        {openFailed && (
          <p className="tv-monb-error" role="alert">
            {openFailedCopy(tr)}
          </p>
        )}
      </section>
      <footer className="tv-monb-foot">
        {/* The privacy statement stands right above the jump. */}
        <div className="tv-monb-privacy">
          <span aria-hidden="true" className="tv-monb-privacy-bar" />
          <span className="tv-monb-privacy-text">
            {tr('Typvia never records what you type.', 'Typvia 从不记录你键入的内容。')}
            <br />
            {tr('Nothing leaves the device unencrypted.', '未经加密的数据不会离开设备。')}
          </span>
        </div>
        <button type="button" className="tv-monb-primary" onClick={openSettings}>
          {tr('Open Settings', '打开设置')}
        </button>
        <button type="button" className="tv-monb-skip" onClick={onContinue}>
          {tr("Nothing happened? I'll do it later — continue", '没有反应?稍后再设 —— 继续')}
        </button>
      </footer>
    </>
  );
}

/**
 * Android branch, merged into the shared spine: enabling walks two system
 * screens (the jump is a real ACTION_INPUT_METHOD_SETTINGS intent), then
 * switching is rehearsed against
 * a real text line — focusing it raises the user's actual keyboard with the
 * real system switcher affordance, which is the honest version of the
 * design's drawn QWERTY. The step names Android's generic input-method
 * warning here, where it will actually appear (Android has no Full Access
 * concept, so there is no ledger step).
 */
function AndroidKeyboardStep({ onContinue }: KeyboardStepProps) {
  const tr = useTr();
  const [openFailed, setOpenFailed] = useState(false);

  const openSettings = () => {
    openKeyboardSettings().catch(() => {
      setOpenFailed(true);
    });
  };

  return (
    <>
      <section className="tv-monb-body">
        <h1 className="tv-monb-title">{tr('Turn on the Typvia bar.', '开启 Typvia 输入栏。')}</h1>
        <ol className="tv-monb-path" aria-label={tr('Settings path', '设置路径')}>
          <li>
            {tr(
              'Settings → System → Languages & input → On-screen keyboard',
              '设置 → 系统 → 语言和输入法 → 屏幕键盘',
            )}
          </li>
          <li>{tr('Manage keyboards → switch on Typvia', '管理键盘 → 开启 Typvia')}</li>
        </ol>
        <p className="tv-monb-lead">
          {tr(
            'Android will show a standard warning about input methods collecting text. Typvia never records what you type — the keyboard asks for no Android permissions, not even internet — and the source is public.',
            'Android 会显示一条关于输入法可能收集文字的标准警告。Typvia 从不记录你键入的内容 —— 这个键盘不申请任何 Android 权限,连网络权限也没有 —— 而且源代码是公开的。',
          )}
        </p>

        <h2 className="tv-monb-ledger-label">{tr('Then switch to it', '然后切换过去')}</h2>
        <input
          type="text"
          className="tv-monb-testline"
          placeholder={tr('Type here to test', '在这里输入试试')}
          aria-label={tr('Type here to test', '在这里输入试试')}
        />
        <p className="tv-monb-lead">
          {tr(
            "Tap the line above and your current keyboard opens. The small keyboard icon in the corner of the navigation bar switches input methods — pick Typvia there whenever you want the bar. Come back here once it's switched on; this screen doesn't detect it by itself.",
            '点一下上面的输入行,会弹出你当前的键盘。导航栏角落的小键盘图标用来切换输入法 —— 想用输入栏时在那里选择 Typvia。切换好后再回到这里;这个界面不会自行检测。',
          )}
        </p>
        <p className="tv-monb-lead">
          {tr(
            'Without the bar, nothing else is lost: Typvia still saves and copies snippets on this phone.',
            '没有输入栏也不影响其他功能:Typvia 依然可以在这台手机上保存和复制片段。',
          )}
        </p>
        {openFailed && (
          <p className="tv-monb-error" role="alert">
            {openFailedCopy(tr)}
          </p>
        )}
      </section>
      <footer className="tv-monb-foot">
        <button type="button" className="tv-monb-primary" onClick={openSettings}>
          {tr('Open input settings', '打开输入设置')}
        </button>
        <button type="button" className="tv-monb-skip" onClick={onContinue}>
          {tr("Nothing happened? I'll do it later — continue", '没有反应?稍后再设 —— 继续')}
        </button>
      </footer>
    </>
  );
}

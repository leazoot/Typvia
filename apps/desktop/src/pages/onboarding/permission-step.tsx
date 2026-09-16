// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { accessibilityStatus, openAccessibilitySettings } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';
import { TextAction } from '../../paper/kit';
import { StepBody, useEnterAdvances } from './step-kit';

const MAC = typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac');

/**
 * Step 2 — the grant that lets Typvia type into other apps. The page asks the
 * system again whenever the window comes back to the front, so ticking the box
 * in System Settings is noticed without a restart or a button.
 */
export function PermissionStep({ onContinue }: { onContinue: () => void }) {
  const tr = useTr();
  const [granted, setGranted] = useState<boolean | null>(null);
  const [opened, setOpened] = useState<'no' | 'yes' | 'failed'>('no');

  const check = useCallback(() => {
    accessibilityStatus()
      .then(setGranted)
      .catch(() => setGranted(false));
  }, []);

  useEffect(() => {
    check();
    window.addEventListener('focus', check);
    return () => window.removeEventListener('focus', check);
  }, [check]);

  useEnterAdvances(onContinue, granted === true);

  const open = () => {
    openAccessibilitySettings()
      .then(() => setOpened('yes'))
      .catch(() => setOpened('failed'));
  };

  if (granted === true) {
    return (
      <StepBody
        mascot="happy"
        title={tr('It can type for you now.', '它已经能替你打字了。')}
        actions={
          <TextAction primary onClick={onContinue}>
            {tr('Next', '下一步')}
          </TextAction>
        }
      >
        <p>
          {MAC
            ? tr(
                'Typvia is ticked under Accessibility. Inserts land straight in the app you are in.',
                '「辅助功能」里已经给 Typvia 打上勾了。插入会直接落到你正在用的应用里。',
              )
            : tr(
                'Inserting needs no extra grant on this computer.',
                '这台电脑上插入不需要额外授权。',
              )}
        </p>
      </StepBody>
    );
  }

  return (
    <StepBody
      mascot="confused"
      title={tr('Let it type for you.', '让它能替你打字。')}
      actions={
        <>
          <TextAction primary onClick={open}>
            {tr('Open that page', '打开那一页')}
          </TextAction>
          <TextAction onClick={onContinue}>
            {tr('Skip · just copy to the clipboard for now', '跳过 · 先只用复制到剪贴板')}
          </TextAction>
        </>
      }
    >
      <p>
        {tr(
          'macOS wants Typvia ticked under System Settings → Privacy & Security → Accessibility. No restart once it is — come back and it works.',
          'macOS 要你在「系统设置 → 隐私与安全性 → 辅助功能」里给 Typvia 打上勾。打完不用重启,回来就能用。',
        )}
      </p>
      <p>
        {tr(
          'Without it Typvia still works: what you insert goes to the clipboard for you to paste.',
          '不授权也能用:插入的内容会放进剪贴板,由你自己粘贴。',
        )}
      </p>
      {opened === 'yes' && (
        <p className="tvo-said" role="status">
          {tr(
            'Once it is ticked, come back here — this page notices by itself.',
            '打上勾以后回到这里,这一页会自己发现。',
          )}
        </p>
      )}
      {opened === 'failed' && (
        <p className="tvo-said" role="status">
          {tr(
            'That page did not open. You can find it yourself under System Settings → Privacy & Security → Accessibility.',
            '那一页没打开。可以自己去「系统设置 → 隐私与安全性 → 辅助功能」找到它。',
          )}
        </p>
      )}
    </StepBody>
  );
}

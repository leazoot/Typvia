// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { snippetConvertToSensitive } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { TextAction } from '../../paper/kit';
import { useVault } from '../../vault/vault-context';
import type { Draft } from './use-editor-draft';

/** Human words for detection codes (colour never carries this alone). */
const KIND_WORDS: Record<string, readonly [string, string]> = {
  pem_private_key: ['private key', '私钥'],
  jwt: ['JWT', 'JWT'],
  bearer_token: ['bearer token', 'Bearer 令牌'],
  aws_access_key: ['AWS key', 'AWS 密钥'],
  github_token: ['GitHub token', 'GitHub 令牌'],
  api_key: ['API key', 'API 密钥'],
  db_connection_string: ['database credentials', '数据库凭据'],
  cookie: ['cookie', 'Cookie'],
  high_entropy_string: ['high-entropy string', '高熵字符串'],
  password_field: ['password', '密码'],
};

function kindWord(kind: string, tr: Tr): string {
  const pair = KIND_WORDS[kind];
  return pair === undefined ? kind : tr(...pair);
}

/**
 * Turning a snippet into a vault secret: it encrypts the current body, drops
 * its plaintext from search and history, and hands it to the vault. It needs a
 * saved snippet and an unlocked vault, and says so when either is missing.
 * The confirmation is inline; the only modal on the desktop is elsewhere.
 */
export function VaultMove({
  draft,
  sensitiveKinds,
  saved,
}: {
  draft: Draft;
  sensitiveKinds: readonly string[];
  saved: boolean;
}) {
  const tr = useTr();
  const navigate = useNavigate();
  const { status: vault } = useVault();
  const [confirming, setConfirming] = useState(false);
  const [working, setWorking] = useState(false);
  const [failed, setFailed] = useState(false);

  const initialized = vault?.initialized === true;
  const unlocked = vault?.unlocked === true;
  const openVault = (
    <button type="button" className="tve-link" onClick={() => void navigate('/vault')}>
      {tr('open the vault', '打开保险库')}
    </button>
  );

  const convert = async () => {
    if (draft.id === null || working) return;
    setWorking(true);
    setFailed(false);
    try {
      await snippetConvertToSensitive(draft.id);
      void navigate('/vault');
    } catch {
      setFailed(true);
      setWorking(false);
      setConfirming(false);
    }
  };

  return (
    <div className="tve-vault">
      {!saved || draft.id === null ? (
        <p className="tve-note">
          {tr(
            'Save this snippet before moving it to the vault.',
            '先让它存进本地库,再移进保险库。',
          )}
        </p>
      ) : !initialized ? (
        <p className="tve-note">
          {tr('Set up the vault first —', '先建好保险库 ——')} {openVault}
          {tr('.', '。')}
        </p>
      ) : !unlocked ? (
        <p className="tve-note">
          {tr('Unlock the vault to move this in —', '解锁保险库后就能移进去 ——')} {openVault}
          {tr('.', '。')}
        </p>
      ) : confirming ? (
        <>
          <p className="tve-note">
            {tr(
              'This encrypts the value and removes its plaintext from search and version history.',
              '这会加密正文,并把明文从搜索和版本历史里拿掉。',
            )}
          </p>
          <div className="tve-vault-actions">
            <TextAction primary onClick={() => void convert()}>
              {tr('Move to vault', '移进保险库')}
            </TextAction>
            <button type="button" className="tpi-note-no" onClick={() => setConfirming(false)}>
              {tr('Not now', '先别')}
            </button>
          </div>
        </>
      ) : (
        <>
          <TextAction onClick={() => setConfirming(true)}>
            {tr('Move to the vault', '移到保险库')}
          </TextAction>
          <p className="tve-note is-hint">
            {tr(
              'A vault secret is encrypted, masked, and never sent to AI.',
              '进了保险库就加密、遮住,也永远不会发给 AI。',
            )}
          </p>
        </>
      )}
      {failed && (
        <p className="tve-note" role="alert">
          {tr(
            'Your snippet is unchanged and safe — the move did not go through. Try again.',
            '片段没有变,也很安全 —— 这次没移进去,可以再试一次。',
          )}
        </p>
      )}
      {sensitiveKinds.length > 0 && (
        <p className="tve-note is-hint">
          {tr(
            `Looks like this may contain: ${sensitiveKinds.map((kind) => kindWord(kind, tr)).join(' · ')}.`,
            `看起来可能含有:${sensitiveKinds.map((kind) => kindWord(kind, tr)).join(' · ')}。`,
          )}{' '}
          {initialized && unlocked
            ? tr('Moving it to the vault encrypts and masks it.', '移进保险库就会加密并遮住。')
            : tr(
                'Saving keeps it as normal text — this is only a suggestion.',
                '存下来仍是普通文字 —— 这只是一个提醒。',
              )}
        </p>
      )}
    </div>
  );
}

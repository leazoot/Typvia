// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The vault is the same paper one step darker — that is how it says "this
 * place is different", without a different product around it. Four states
 * off the vault session: waiting, not set up, locked (in place, never a
 * modal), unlocked.
 */
import { useTr } from '@typvia/ui';
import { useLocation } from 'react-router';
import { Mascot } from '../../paper/mascot';
import { useVault } from '../../vault/vault-context';
import { VaultFirstRun } from './first-run';
import { LockedPanel } from './locked-panel';
import { VaultUnlocked } from './unlocked';
import './vault.css';

export function VaultPage() {
  const { status, apply, unlockBiometric, lock } = useVault();
  const tr = useTr();
  const location = useLocation();

  return (
    <div className="tpi tvv">
      <div className="tvv-sheet">
        {status === null ? (
          <section className="tvv-card" role="status">
            <div className="tvv-card-head">
              <Mascot state="thinking" size={32} />
              <span className="tpi-eyebrow">{tr('Vault', '保险库')}</span>
            </div>
          </section>
        ) : !status.initialized ? (
          <VaultFirstRun onInitialized={apply} />
        ) : !status.unlocked ? (
          <LockedPanel
            status={status}
            onUnlocked={apply}
            onUnlockBiometric={unlockBiometric}
            onReset={apply}
          />
        ) : (
          <VaultUnlocked
            status={status}
            onLock={lock}
            startCreating={
              typeof location.state === 'object' &&
              location.state !== null &&
              'create' in location.state
            }
          />
        )}
      </div>
    </div>
  );
}

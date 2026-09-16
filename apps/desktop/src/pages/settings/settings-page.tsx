// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Settings: one page to the end. Groups are only their titles — no cards,
 * no folds, no save button; every choice takes effect as it is made and the
 * specimens beside it show what it means.
 */
import { getVersion } from '@tauri-apps/api/app';
import { useEffect, useState } from 'react';
import { Mascot } from '../../paper/mascot';
import { useTitleBar } from '../../workspace/title-bar';
import { AppRulesGroup } from './group-app-rules';
import { DataGroup } from './group-data';
import { GeneralGroup } from './group-general';
import { InsertionGroup } from './group-insertion';
import { LookGroups } from './group-look';
import { ProviderGroup } from './group-provider';
import { SemanticGroup } from './group-semantic';
import { ShortcutsGroup } from './group-shortcuts';
import { SyncGroup } from './group-sync';
import { useLibraryTotal } from './use-settings-facts';
import './settings-page.css';

export function SettingsPage() {
  const total = useLibraryTotal();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  useTitleBar(
    {
      trailing: (
        <span className="tvs-version">
          <Mascot state="idle" size={21} />
          {version !== null && <span className="tvs-version-text">{version}</span>}
        </span>
      ),
    },
    `settings-${version ?? ''}`,
  );

  return (
    <div className="tpi tvs">
      <div className="tvs-sheet">
        <GeneralGroup />
        <InsertionGroup />
        <SyncGroup />
        <ShortcutsGroup />
        <DataGroup />
        <LookGroups />
        <SemanticGroup />
        <ProviderGroup />
        <AppRulesGroup total={total} />
      </div>
    </div>
  );
}

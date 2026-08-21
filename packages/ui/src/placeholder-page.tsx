// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from './i18n';

interface PlaceholderPageProps {
  labelEn: string;
  labelCn: string;
}

/**
 * Unstyled stand-in body for screens that are not implemented yet.
 * Replaced page by page; carries no design values on purpose — styling
 * arrives with the token system.
 */
export function PlaceholderPage({ labelEn, labelCn }: PlaceholderPageProps) {
  const tr = useTr();
  return (
    <section>
      <h1>{tr(labelEn, labelCn)}</h1>
      <p>{tr('This screen is not implemented yet.', '该界面尚未实现。')}</p>
    </section>
  );
}

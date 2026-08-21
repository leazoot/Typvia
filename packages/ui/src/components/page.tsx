// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { ReactNode } from 'react';
import './page.css';

interface PageProps {
  children: ReactNode;
}

/** Desktop page container: the 40px page padding on the paper surface. */
export function Page({ children }: PageProps) {
  return <main className="tv-page">{children}</main>;
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The type ships inside the app. A local-first product does not ask a font CDN
// for its letters at launch, and the webview's CSP would refuse the request
// anyway. The CJK families are split by unicode-range, so a page only loads
// the slices its characters actually need.
import '@fontsource-variable/source-serif-4/opsz.css';
import '@fontsource-variable/noto-serif-sc';
import '@fontsource-variable/noto-sans-sc';
import '@fontsource-variable/instrument-sans';
import '@fontsource/ibm-plex-sans/400.css';
import '@fontsource/ibm-plex-sans/500.css';
import '@fontsource/ibm-plex-sans/600.css';
import '@fontsource/ibm-plex-mono/400.css';
import '@fontsource/ibm-plex-mono/500.css';

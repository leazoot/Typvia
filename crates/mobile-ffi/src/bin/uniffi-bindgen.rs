// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Library-mode bindgen entry point: bindings are generated from
// compiled library metadata, so no UDL file exists to drift out of sync.
fn main() {
    uniffi::uniffi_bindgen_main()
}

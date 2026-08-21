// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Real-espanso detection. Ignored by default (requires espanso installed on
//! the host, like the injector live tests). Run with:
//!
//! ```text
//! cargo test -p typvia-espanso-adapter --test detection_live -- --ignored
//! ```

use typvia_espanso_adapter::{EspansoAdapter, EspansoStatus, SystemEspansoCli};

#[test]
#[ignore = "requires espanso installed on the host"]
fn detects_real_espanso_install_and_paths() {
    let adapter = EspansoAdapter::new(SystemEspansoCli);

    let version = match adapter.status() {
        EspansoStatus::NotInstalled => {
            panic!("espanso is expected to be installed in this environment")
        }
        EspansoStatus::Stopped { version } | EspansoStatus::Running { version } => version,
    };
    assert!(
        version.major().is_some(),
        "version should parse from real output: {:?}",
        version.raw()
    );

    let paths = adapter.paths().expect("espanso path should resolve");
    assert!(
        paths.config.is_absolute(),
        "config path: {:?}",
        paths.config
    );
    assert!(
        paths.runtime.is_absolute(),
        "runtime path: {:?}",
        paths.runtime
    );
}

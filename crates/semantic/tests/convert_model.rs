// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Utility runner: converts a downloaded fp32 model directory to the
//! product's f16 serving layout (same conversion the desktop download
//! pipeline performs). Env-gated and ignored so normal runs never touch
//! it; run explicitly with:
//! `TYPVIA_E5_DIR=/path cargo test -p typvia-semantic --test convert_model -- --ignored`

#![allow(clippy::unwrap_used)]

#[test]
#[ignore = "utility conversion over a locally downloaded model; set TYPVIA_E5_DIR"]
fn convert_downloaded_model_to_f16() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("TYPVIA_E5_DIR").expect("set TYPVIA_E5_DIR to the model directory"),
    );
    let input = dir.join("model.safetensors");
    let output = dir.join("model.f16.safetensors");
    typvia_semantic::convert_weights_to_f16(&input, &output).unwrap();
    let in_len = std::fs::metadata(&input).unwrap().len();
    let out_len = std::fs::metadata(&output).unwrap().len();
    println!("f16 conversion: {in_len} -> {out_len} bytes");
    assert!(out_len < in_len / 2 + in_len / 8);
}

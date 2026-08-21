// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Real-espanso config validation. Ignored by default (requires espanso
//! installed on the host). Compiles a small snippet set, writes it into the
//! isolated `match/typvia/` directory, and asserts `espanso match list` parses
//! it — while confirming a sensitive snippet never reaches the file. Run with:
//!
//! ```text
//! cargo test -p typvia-espanso-adapter --test compile_live -- --ignored
//! ```

use typvia_core::model::{SecurityLevel, Snippet, SnippetContent, SnippetType, TriggerMode};
use typvia_espanso_adapter::{
    EspansoAdapter, EspansoCli, SystemEspansoCli, compile_snippets, write_config,
};

fn snippet(id: &str, trigger: &str, level: SecurityLevel, content: SnippetContent) -> Snippet {
    Snippet {
        id: id.to_string(),
        workspace_id: "w1".to_string(),
        title: "t".to_string(),
        content,
        snippet_type: SnippetType::Text,
        description: None,
        folder_id: None,
        trigger: Some(trigger.to_string()),
        trigger_mode: Some(TriggerMode::Immediate),
        language: None,
        security_level: level,
        is_favorite: false,
        is_pinned: false,
        is_enabled: true,
        platform_scope: vec![],
        created_at: 1,
        updated_at: 1,
        last_used_at: None,
        usage_count: 0,
        version: 1,
        deleted_at: None,
        conflict_of: None,
    }
}

#[test]
#[ignore = "requires espanso installed on the host"]
fn generates_config_that_espanso_parses_without_the_sensitive_snippet() {
    let adapter = EspansoAdapter::new(SystemEspansoCli);
    let paths = adapter.paths().expect("espanso path should resolve");
    let target = paths.typvia_config_path();

    let normal = snippet(
        "normal",
        ":typviaok",
        SecurityLevel::Normal,
        SnippetContent::Plaintext("Typvia expansion works".to_string()),
    );
    let sensitive = snippet(
        "sensitive",
        ":typviasecret",
        SecurityLevel::Sensitive,
        SnippetContent::Ciphertext(b"ciphertext".to_vec()),
    );

    let compiled = compile_snippets(&[normal, sensitive]).expect("compiles");
    assert_eq!(
        compiled.match_count, 1,
        "only the normal snippet is included"
    );

    write_config(&target, &compiled.yaml).expect("write config");
    let listing = SystemEspansoCli.run(&["match", "list"]);

    // Clean up before asserting so a failure never leaves the isolated file.
    // Only our single file is removed; espanso's own match/ dir is left alone.
    let _ = std::fs::remove_file(&target);

    let listing = listing.expect("espanso match list runs");
    assert_eq!(
        listing.code,
        Some(0),
        "match list stderr: {}",
        listing.stderr
    );
    assert!(
        listing.stdout.contains(":typviaok"),
        "expected trigger missing from: {}",
        listing.stdout
    );
    assert!(
        !listing.stdout.contains(":typviasecret"),
        "sensitive trigger reached espanso"
    );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! String-backed enums stored as controlled TEXT values in SQLite.

use std::fmt;
use std::str::FromStr;

/// A TEXT value read from storage (or sync payload) that no known enum
/// variant matches. Newer schema versions may introduce values this build
/// does not know; callers must decide explicitly how to degrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEnumValue {
    /// Name of the enum type that failed to parse.
    pub enum_name: &'static str,
    /// The unrecognized TEXT value.
    pub value: String,
}

impl fmt::Display for UnknownEnumValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown {} value: {}", self.enum_name, self.value)
    }
}

impl std::error::Error for UnknownEnumValue {}

macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every known variant, in declaration order.
            pub const ALL: &'static [$name] = &[$(Self::$variant),+];

            /// Stable TEXT value stored in the database.
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl FromStr for $name {
            type Err = UnknownEnumValue;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    other => Err(UnknownEnumValue {
                        enum_name: stringify!($name),
                        value: other.to_string(),
                    }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

text_enum! {
    /// Snippet content type.
    SnippetType {
        /// Plain text.
        Text => "text",
        /// Markdown document.
        Markdown => "markdown",
        /// Code fragment (with an optional `language`).
        Code => "code",
        /// Shell or CLI command.
        Command => "command",
        /// AI prompt text.
        Prompt => "prompt",
        /// Template with fillable fields (see `TemplateField`).
        Template => "template",
        /// Sensitive text; requires `SecurityLevel::Sensitive`.
        Sensitive => "sensitive",
        /// Stored AI action invocation.
        AiAction => "ai_action",
        /// URL / link.
        Link => "link",
        /// Temporary snippet; expiry semantics are undecided and not implemented.
        Temporary => "temporary",
    }
}

text_enum! {
    /// Content security level.
    ///
    /// Sensitive content is ciphertext-only at rest, indexed by
    /// title/tags/description only, and never enters Espanso YAML or logs.
    SecurityLevel {
        /// Plaintext storage, full-text indexed, direct injection.
        Normal => "normal",
        /// Application-layer encrypted, metadata-only index, verify-then-inject.
        Sensitive => "sensitive",
    }
}

text_enum! {
    /// How a trigger string fires expansion.
    ///
    /// Case sensitivity, cursor placeholders, and date variables are
    /// compile-time options of the Espanso adapter, not persisted trigger
    /// modes.
    TriggerMode {
        /// Expands after the trigger plus a terminator character.
        Delimiter => "delimiter",
        /// Expands as soon as the trigger is typed.
        Immediate => "immediate",
        /// Expands only when the trigger forms a whole word.
        WordBoundary => "word_boundary",
        /// The trigger string is a regular expression. Pattern validity is
        /// checked by the Espanso adapter at compile time, not by the model.
        Regex => "regex",
    }
}

text_enum! {
    /// Supported platforms.
    Platform {
        Windows => "windows",
        Macos => "macos",
        Ios => "ios",
        Android => "android",
    }
}

text_enum! {
    /// Template field input type.
    TemplateFieldType {
        SingleLineText => "single_line_text",
        MultiLineText => "multi_line_text",
        Number => "number",
        Date => "date",
        Time => "time",
        /// Single choice from `options`.
        SingleSelect => "single_select",
        /// Multiple choices from `options`.
        MultiSelect => "multi_select",
        Toggle => "toggle",
        /// Dropdown choice from `options`.
        Dropdown => "dropdown",
        /// Value produced at render time (date, clipboard, ...).
        DynamicVariable => "dynamic_variable",
        /// Reference to a vault secret; only the reference is stored, never
        /// the value (cross-domain rule).
        SecretRef => "secret_ref",
    }
}

text_enum! {
    /// Per-snippet application rule effect.
    ///
    /// The app-level "show only tagged snippets" rule has no snippet-scoped
    /// representation and remains undecided.
    AppRuleType {
        /// Show this snippet only in the matched application.
        ShowOnly => "show_only",
        /// Hide this snippet in the matched application.
        Disable => "disable",
        /// Keep the snippet visible but disable abbreviation expansion.
        DisableExpansion => "disable_expansion",
        /// Forbid sensitive injection into the matched application.
        DenySensitiveInjection => "deny_sensitive_injection",
    }
}

text_enum! {
    /// Device trust state.
    TrustLevel {
        /// Paired and allowed to sync.
        Trusted => "trusted",
        /// Revoked; the server rejects it and keys are rotated.
        Revoked => "revoked",
    }
}

text_enum! {
    /// Key domain for the vault key hierarchy.
    ///
    /// Each domain has an independently generated key wrapped under the MK, so
    /// one domain can rotate without touching the other.
    KeyDomain {
        /// Normal-snippet sync domain (`K_sync`).
        Sync => "sync",
        /// Sensitive vault domain (`K_vault`).
        Vault => "vault",
    }
}

text_enum! {
    /// Which sync backend the account is bound to.
    TransportKind {
        /// The Go sync server: global server_seq, push-time 409s.
        Server => "server",
        /// A user-owned WebDAV endpoint as dumb encrypted storage.
        Webdav => "webdav",
    }
}

/// A fresh install is the server form until the user binds a WebDAV
/// endpoint; Default keeps `SyncConfig::default()` meaningful.
impl Default for TransportKind {
    fn default() -> Self {
        Self::Server
    }
}

text_enum! {
    /// Entity kind carried by a sync record.
    SyncEntityType {
        Snippet => "snippet",
        TemplateField => "template_field",
        Folder => "folder",
        Tag => "tag",
        SnippetTag => "snippet_tag",
        AppRule => "app_rule",
        AiAction => "ai_action",
    }
}

text_enum! {
    /// Outbound lifecycle of a queued sync record.
    OutboxState {
        /// Sealed locally, not yet accepted by the server.
        Pending => "pending",
        /// Accepted by the server; `server_seq` is recorded.
        Applied => "applied",
        /// Rejected with a version conflict (409); parked until the
        /// three-way merge re-pushes it. Never dropped, never faked as
        /// success.
        Conflict => "conflict",
    }
}

text_enum! {
    /// Why a pulled remote record is parked instead of applied.
    PendingReason {
        /// Sealed under a K_sync generation this client does not hold yet.
        UnknownKeyId => "unknown_key_id",
        /// Payload document version is newer than this build supports.
        PayloadAhead => "payload_ahead",
        /// Entity type without a local apply path in this build.
        UnsupportedEntity => "unsupported_entity",
    }
}

text_enum! {
    /// Where an AI action takes its input text from.
    AiActionInputSource {
        /// The current selection in the target app or editor.
        Selection => "selection",
        /// The clipboard text at invocation time.
        Clipboard => "clipboard",
        /// The body of the snippet the action is invoked on.
        Snippet => "snippet",
        /// Text handed in via the mobile share sheet.
        Share => "share",
    }
}

text_enum! {
    /// What happens with an AI action's result — always after the user
    /// confirms it.
    AiActionOutputMode {
        /// Replace the input in place.
        Replace => "replace",
        /// Insert at the cursor, leaving the input untouched.
        Insert => "insert",
        /// Copy the result to the clipboard.
        Copy => "copy",
        /// Save the result as a new snippet draft.
        NewSnippet => "new_snippet",
    }
}

text_enum! {
    /// How an AI action treats secret-bearing input. Sensitive snippets are
    /// refused under every scope — no value relaxes that red line, by design.
    AiActionPermissionScope {
        /// Refuse to run when the input carries a suspected secret.
        NormalOnly => "normal_only",
        /// Mask suspected secret substrings before sending.
        MaskSecrets => "mask_secrets",
    }
}

text_enum! {
    /// Configured AI provider form. The wire
    /// format is OpenAI-compatible for every kind; `crates/ai` reuses this
    /// enum as its configuration vocabulary.
    AiProviderKind {
        /// Local Ollama daemon.
        Ollama => "ollama",
        /// Local LM Studio server.
        LmStudio => "lm_studio",
        /// A hosted OpenAI-compatible service (BYOK).
        OpenAiCompatible => "openai_compatible",
        /// Any other OpenAI-compatible endpoint the user points at.
        CustomBaseUrl => "custom_base_url",
    }
}

text_enum! {
    /// What kind of AI request left this device: the
    /// `ai_egress_log.request_class` vocabulary.
    /// `crates/ai` reuses this enum on the egress path.
    AiRequestClass {
        /// A `/chat/completions` call carrying prompt content.
        Completion => "completion",
        /// A `/models` reachability probe with no request body.
        Connectivity => "connectivity",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_round_trip<T>(variants: &[T])
    where
        T: FromStr<Err = UnknownEnumValue> + PartialEq + Copy + fmt::Debug,
        T: fmt::Display,
    {
        for v in variants {
            let text = v.to_string();
            assert_eq!(text.parse::<T>(), Ok(*v), "round-trip failed for {text}");
        }
    }

    #[test]
    fn every_variant_round_trips_through_its_text_value() {
        assert_round_trip(SnippetType::ALL);
        assert_round_trip(SecurityLevel::ALL);
        assert_round_trip(TriggerMode::ALL);
        assert_round_trip(Platform::ALL);
        assert_round_trip(TemplateFieldType::ALL);
        assert_round_trip(AppRuleType::ALL);
        assert_round_trip(TrustLevel::ALL);
        assert_round_trip(KeyDomain::ALL);
        assert_round_trip(TransportKind::ALL);
        assert_round_trip(SyncEntityType::ALL);
        assert_round_trip(OutboxState::ALL);
        assert_round_trip(PendingReason::ALL);
        assert_round_trip(AiActionInputSource::ALL);
        assert_round_trip(AiActionOutputMode::ALL);
        assert_round_trip(AiActionPermissionScope::ALL);
        assert_round_trip(AiProviderKind::ALL);
        assert_round_trip(AiRequestClass::ALL);
    }

    #[test]
    fn snippet_type_covers_all_ten_prd_types() {
        assert_eq!(SnippetType::ALL.len(), 10);
    }

    #[test]
    fn unknown_value_is_reported_with_enum_name_and_value() {
        let err = "vault".parse::<SecurityLevel>();
        assert_eq!(
            err,
            Err(UnknownEnumValue {
                enum_name: "SecurityLevel",
                value: "vault".to_string(),
            })
        );
    }

    #[test]
    fn text_values_are_snake_case_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for v in SnippetType::ALL {
            let s = v.as_str();
            assert!(seen.insert(s), "duplicate text value {s}");
            assert_eq!(s, s.to_lowercase());
            assert!(!s.contains(' '));
        }
    }
}

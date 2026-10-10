//! Behavior tests for the parent module, kept separate from runtime code.

use super::fake::FakeStore;
use super::*;
use serde::de::IntoDeserializer;
use serde::de::value::Error as ValueError;

fn secret(value: &str) -> Secret {
    Secret::new(value.to_string())
}

#[test]
fn secret_is_read_from_a_string_field_only() {
    let parsed = Secret::deserialize(IntoDeserializer::<ValueError>::into_deserializer(
        "synthetic-test-value",
    ))
    .expect("a string is accepted");
    assert_eq!(parsed.expose(), "synthetic-test-value");
    assert_eq!(format!("{parsed:?}"), "Secret(<redacted>)");

    let number = Secret::deserialize(IntoDeserializer::<ValueError>::into_deserializer(5_u32));
    assert!(number.is_err());
}

#[test]
fn secrets_are_trimmed_and_any_printable_text_is_allowed() {
    for (input, expected) in [
        ("sk-synthetic", "sk-synthetic"),
        ("  sk-synthetic \n", "sk-synthetic"),
        ("\u{3000}key\u{3000}", "key"),
        ("inner space key", "inner space key"),
        ("密钥-é-ключ-🔑", "密钥-é-ключ-🔑"),
    ] {
        let validated = validate_secret(secret(input)).expect("valid secret");
        assert_eq!(validated.expose(), expected, "input: {input:?}");
    }
}

#[test]
fn empty_or_control_character_secrets_are_rejected() {
    for input in [
        "",
        "   ",
        " \t\r\n ",
        "line\nbreak",
        "carriage\rreturn",
        "inner\ttab",
        "nul\u{0}byte",
        "delete\u{7F}char",
        "next\u{85}line",
    ] {
        assert_eq!(
            validate_secret(secret(input)).map(|_| ()),
            Err(InvalidSecret),
            "input: {input:?}"
        );
    }
}

#[test]
fn secret_length_is_limited_by_utf16_units_after_trimming() {
    let ascii_limit = "a".repeat(MAX_SECRET_UTF16_UNITS);
    let bmp_limit = "密".repeat(MAX_SECRET_UTF16_UNITS);
    // Each emoji outside the Basic Multilingual Plane takes two UTF-16 units.
    let emoji_limit = "🔑".repeat(MAX_SECRET_UTF16_UNITS / 2);
    let padded = format!("  {ascii_limit}\n");
    for valid in [&ascii_limit, &bmp_limit, &emoji_limit, &padded] {
        assert!(validate_secret(secret(valid)).is_ok());
    }
    for too_long in [
        format!("{ascii_limit}a"),
        format!("{bmp_limit}密"),
        format!("{emoji_limit}🔑"),
        format!("{emoji_limit}a"),
    ] {
        assert_eq!(
            validate_secret(secret(&too_long)).map(|_| ()),
            Err(InvalidSecret)
        );
    }
}

#[test]
fn invalid_secret_error_never_contains_the_value() {
    let error = validate_secret(secret("synthetic\nvalue")).map(|_| ());
    let error = error.expect_err("rejected");
    assert!(!format!("{error:?} {error}").contains("synthetic"));
}

#[test]
fn secret_debug_output_hides_the_value() {
    let secret = Secret::new("synthetic-test-value".to_string());

    assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
}

#[test]
fn unavailable_store_stops_before_the_database_commit() {
    let store = FakeStore::holding("provider-1", "old");
    store.fail_saves(Some(CredentialError::Unavailable));
    let mut committed = false;

    let result = replace_then_commit(
        &store,
        "provider-1",
        &Secret::new("new".to_string()),
        || {
            committed = true;
            Ok::<(), CommitFailure<&'static str>>(())
        },
    );

    assert!(matches!(
        result,
        Err(ReplaceError::Credential(CredentialError::Unavailable))
    ));
    assert!(!committed);
    assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
}

#[test]
fn access_denied_during_replace_keeps_the_previous_secret() {
    let store = FakeStore::holding("provider-1", "old");
    store.fail_saves(Some(CredentialError::AccessDenied));

    let result = replace_then_commit(
        &store,
        "provider-1",
        &Secret::new("new".to_string()),
        || Ok::<(), CommitFailure<&'static str>>(()),
    );

    assert!(matches!(
        result,
        Err(ReplaceError::Credential(CredentialError::AccessDenied))
    ));
    assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
}

#[test]
fn failed_commit_restores_the_previous_secret() {
    let store = FakeStore::holding("provider-1", "old");

    let result = replace_then_commit(
        &store,
        "provider-1",
        &Secret::new("new".to_string()),
        || Err::<(), _>(CommitFailure::NotCommitted("database write failed")),
    );

    assert!(matches!(
        result,
        Err(ReplaceError::CommitFailed { restored: true, .. })
    ));
    assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
}

#[test]
fn failed_commit_removes_a_new_secret_when_none_existed_before() {
    let store = FakeStore::default();

    let result = replace_then_commit(
        &store,
        "provider-1",
        &Secret::new("new".to_string()),
        || Err::<(), _>(CommitFailure::NotCommitted("database write failed")),
    );

    assert!(matches!(
        result,
        Err(ReplaceError::CommitFailed { restored: true, .. })
    ));
    assert_eq!(store.stored("provider-1"), None);
}

#[test]
fn failed_restore_is_reported_as_not_restored() {
    let store = FakeStore::holding("provider-1", "old");

    let result = replace_then_commit(
        &store,
        "provider-1",
        &Secret::new("new".to_string()),
        || {
            // Simulate the store failing again during the compensation step.
            store.fail_saves(Some(CredentialError::OperationFailed));
            Err::<(), _>(CommitFailure::NotCommitted("database write failed"))
        },
    );

    assert!(matches!(
        result,
        Err(ReplaceError::CommitFailed {
            restored: false,
            ..
        })
    ));
    assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
}

#[test]
fn failed_delete_during_restore_is_reported() {
    let store = FakeStore::default();

    let result = replace_then_commit(
        &store,
        "provider-1",
        &Secret::new("new".to_string()),
        || {
            store.fail_deletes(Some(CredentialError::OperationFailed));
            Err::<(), _>(CommitFailure::NotCommitted("database write failed"))
        },
    );

    assert!(matches!(
        result,
        Err(ReplaceError::CommitFailed {
            restored: false,
            ..
        })
    ));
    assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
}

#[test]
fn replace_returns_the_commit_value_and_keeps_the_new_secret() {
    let store = FakeStore::holding("provider-1", "old");

    let result = replace_then_commit(&store, "provider-1", &secret("new"), || {
        Ok::<_, CommitFailure<&'static str>>(42)
    });

    assert!(matches!(result, Ok(42)));
    assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
}

#[test]
fn unreadable_backup_stops_before_writing_unless_it_is_corrupt() {
    let store = FakeStore::holding("provider-1", "old");
    store.fail_loads(Some(CredentialError::AccessDenied));
    let mut committed = false;

    let result = replace_then_commit(&store, "provider-1", &secret("new"), || {
        committed = true;
        Ok::<(), CommitFailure<&'static str>>(())
    });

    assert!(matches!(
        result,
        Err(ReplaceError::Credential(CredentialError::AccessDenied))
    ));
    assert!(!committed);
    // Only the failed backup read reached the store; nothing was written.
    assert_eq!(store.call_count(), 1);
    store.fail_loads(None);
    assert_eq!(store.stored("provider-1").as_deref(), Some("old"));
}

#[test]
fn corrupt_previous_secret_can_be_replaced() {
    let store = FakeStore::holding("provider-1", "old");
    store.fail_loads(Some(CredentialError::CorruptValue));

    let result = replace_then_commit(&store, "provider-1", &secret("new"), || {
        Ok::<(), CommitFailure<&'static str>>(())
    });

    assert!(result.is_ok());
    assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
}

#[test]
fn failed_commit_after_a_corrupt_backup_keeps_the_new_secret() {
    let store = FakeStore::holding("provider-1", "old");
    store.fail_loads(Some(CredentialError::CorruptValue));

    let result = replace_then_commit(&store, "provider-1", &secret("new"), || {
        Err::<(), _>(CommitFailure::NotCommitted("database write failed"))
    });

    assert!(matches!(
        result,
        Err(ReplaceError::CommitFailed {
            restored: false,
            ..
        })
    ));
    // Deleting would leave the referenced entry empty, so the new value stays.
    assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
}

#[test]
fn save_new_returns_the_commit_value_and_keeps_the_entry() {
    let store = FakeStore::default();

    let result = save_new_then_commit(&store, "provider-2", &secret("new"), || {
        Ok::<_, CommitFailure<&'static str>>("record")
    });

    assert!(matches!(result, Ok("record")));
    assert_eq!(store.stored("provider-2").as_deref(), Some("new"));
}

#[test]
fn save_new_stops_before_the_commit_when_the_store_fails() {
    let store = FakeStore::default();
    store.fail_saves(Some(CredentialError::Unavailable));
    let mut committed = false;

    let result = save_new_then_commit(&store, "provider-2", &secret("new"), || {
        committed = true;
        Ok::<(), CommitFailure<&'static str>>(())
    });

    assert!(matches!(
        result,
        Err(SaveNewError::Credential(CredentialError::Unavailable))
    ));
    assert!(!committed);
    assert_eq!(store.stored("provider-2"), None);
}

#[test]
fn save_new_deletes_the_entry_when_the_commit_fails() {
    let store = FakeStore::default();

    let result = save_new_then_commit(&store, "provider-2", &secret("new"), || {
        Err::<(), _>(CommitFailure::NotCommitted("database write failed"))
    });

    assert!(matches!(
        result,
        Err(SaveNewError::CommitFailed {
            source: "database write failed",
            cleaned_up: true
        })
    ));
    assert_eq!(store.stored("provider-2"), None);
}

#[test]
fn save_new_reports_an_entry_it_could_not_delete() {
    let store = FakeStore::default();
    store.fail_deletes(Some(CredentialError::OperationFailed));

    let result = save_new_then_commit(&store, "provider-2", &secret("new"), || {
        Err::<(), _>(CommitFailure::NotCommitted("database write failed"))
    });

    assert!(matches!(
        result,
        Err(SaveNewError::CommitFailed {
            cleaned_up: false,
            ..
        })
    ));
    assert_eq!(store.stored("provider-2").as_deref(), Some("new"));
}

#[test]
fn uncertain_commit_keeps_a_new_entry() {
    let store = FakeStore::default();

    let result = save_new_then_commit(&store, "provider-2", &secret("new"), || {
        Err::<(), _>(CommitFailure::Uncertain("commit failed"))
    });

    assert!(matches!(
        result,
        Err(SaveNewError::CommitUncertain {
            source: "commit failed"
        })
    ));
    assert_eq!(store.stored("provider-2").as_deref(), Some("new"));
    // save only; no compensating delete.
    assert_eq!(store.call_count(), 1);
}

#[test]
fn uncertain_commit_keeps_the_replacement_instead_of_restoring() {
    for previous in [Some("old"), None] {
        let store = match previous {
            Some(value) => FakeStore::holding("provider-1", value),
            None => FakeStore::default(),
        };

        let result = replace_then_commit(&store, "provider-1", &secret("new"), || {
            Err::<(), _>(CommitFailure::Uncertain("commit failed"))
        });

        assert!(matches!(
            result,
            Err(ReplaceError::CommitUncertain {
                source: "commit failed"
            })
        ));
        assert_eq!(store.stored("provider-1").as_deref(), Some("new"));
    }
}

#[test]
fn platform_errors_map_to_safe_categories() {
    assert_eq!(
        map_keyring_error(keyring::Error::NoDefaultStore),
        CredentialError::Unavailable
    );
    assert_eq!(
        map_keyring_error(keyring::Error::NoStorageAccess(Box::new(
            std::io::Error::other("locked")
        ))),
        CredentialError::AccessDenied
    );
    assert_eq!(
        map_keyring_error(keyring::Error::PlatformFailure(Box::new(
            std::io::Error::other("platform failure")
        ))),
        CredentialError::OperationFailed
    );
    assert_eq!(
        map_keyring_error(keyring::Error::BadEncoding(b"not-text".to_vec())),
        CredentialError::CorruptValue
    );
    assert_eq!(
        map_keyring_error(keyring::Error::TooLong("account".to_string(), 10)),
        CredentialError::InvalidReference
    );
    assert_eq!(
        map_keyring_error(keyring::Error::Invalid(
            "service".to_string(),
            "empty".to_string()
        )),
        CredentialError::InvalidReference
    );
    assert_eq!(
        map_keyring_error(keyring::Error::BadStoreFormat("broken".to_string())),
        CredentialError::CorruptValue
    );
    assert_eq!(
        map_keyring_error(keyring::Error::Ambiguous(Vec::new())),
        CredentialError::OperationFailed
    );
    assert_eq!(
        map_keyring_error(keyring::Error::NotSupportedByStore("search".to_string())),
        CredentialError::OperationFailed
    );
}

/// Manual check against the real platform store. It creates and deletes a
/// temporary synthetic entry. Run it on a developer machine with:
/// `cargo test --manifest-path src-tauri/Cargo.toml --locked credentials -- --ignored`
#[test]
#[ignore = "touches the real OS credential store; run manually on a developer machine"]
fn os_store_round_trip_uses_a_temporary_entry() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after 1970")
        .as_nanos();
    let reference = format!("smoke-test-{}-{nanos}", std::process::id());
    let store = OsCredentialStore;

    store
        .save(
            &reference,
            &Secret::new("synthetic-smoke-value".to_string()),
        )
        .expect("save temporary entry");
    let loaded = store
        .load(&reference)
        .expect("load temporary entry")
        .expect("temporary entry exists");
    assert_eq!(loaded.expose(), "synthetic-smoke-value");
    store.delete(&reference).expect("delete temporary entry");
    assert!(store.load(&reference).expect("load after delete").is_none());
}

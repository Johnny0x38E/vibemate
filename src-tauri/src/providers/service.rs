//! Provider save orchestration across SQLite and OS credentials, including compensation.

use super::repository::{
    commit_provider_update, generate_provider_id, get_provider, insert_provider_with_reference,
    read_selected_model_count,
};
use super::types::*;
use super::validation::{parse_kind, validate_settings};
use crate::credentials::{
    CommitFailure, CredentialStore, ReplaceError, SaveNewError, Secret, replace_then_commit,
    save_new_then_commit, validate_secret,
};
use crate::storage::Storage;

/// Validate a new instance and its API key, save both, and return the instance.
///
/// The key goes to the OS credential store and the instance to SQLite. The two
/// stores share no transaction, so the steps are ordered to never leave half a
/// provider behind:
/// 1. Validate every field, the key last, before touching either store.
/// 2. Generate the ID with SQLite's `randomblob` (16 random bytes as lowercase
///    hex). This runs outside the insert transaction, and the connection is
///    unlocked again before step 3, because the credential store may show a
///    system prompt and wait for the user.
/// 3. Save the key under `provider-<id>`. If that fails, nothing was saved.
/// 4. In one transaction, insert the instance and its `provider_credential` row.
/// 5. If an `INSERT` in step 4 fails, nothing was committed: delete the key again
///    (`save_new_then_commit`) and report `WriteFailed`. If even that delete
///    fails, report `CreateOutcomeUnknown`: the leftover entry has no reference,
///    so the app never reads it, but the user must check the list.
/// 6. If `COMMIT` itself fails, the provider may have been saved anyway, so the
///    key is kept (deleting it could leave a saved provider without a key) and
///    the result is `CreateOutcomeUnknown`.
///
/// Random IDs, unlike reused integer IDs, cannot point at a credential left
/// behind by an older or restored database, and the primary key rejects the
/// (practically impossible) duplicate instead of overwriting a row.
///
/// Extensions are validated but not stored: every allowlist is empty today, so a
/// validated map is always empty. Storage arrives with the first supported key.
///
/// # Errors
/// Validation codes (including `SecretInvalid`), `StorageUnavailable`,
/// credential-store codes (nothing was saved), `WriteFailed` (nothing was saved),
/// or `CreateOutcomeUnknown`.
pub fn create_provider(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: CreateProviderRequest,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    let kind = parse_kind(&request.kind)?;
    let settings = validate_settings(
        kind,
        &request.display_name,
        &request.base_url,
        &request.protocol,
        &request.extensions,
    )?;
    let secret = validate_secret(request.secret)?;
    let id = generate_provider_id(storage)?;
    let reference = id.credential_reference();
    let saved = save_new_then_commit(store, &reference, &secret, || {
        insert_provider_with_reference(storage, &id, kind, &settings, &reference, now_ms)
    });
    match saved {
        Ok(()) => {
            let connection = storage
                .lock()
                .map_err(|_| ProviderError::StorageUnavailable)?;
            Ok(ProviderRecord {
                id: id.clone(),
                kind,
                display_name: settings.display_name,
                base_url: settings.base_url,
                protocol: settings.protocol,
                extensions: settings.extensions,
                revision: 1,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
                selected_model_count: read_selected_model_count(&connection, &id)?,
            })
        }
        Err(SaveNewError::Credential(error)) => Err(error.into()),
        // The key was deleted again, so the SQLite error describes the whole outcome.
        Err(SaveNewError::CommitFailed {
            source,
            cleaned_up: true,
        }) => Err(source),
        Err(SaveNewError::CommitFailed {
            cleaned_up: false, ..
        })
        | Err(SaveNewError::CommitUncertain { .. }) => Err(ProviderError::CreateOutcomeUnknown),
    }
}

/// Edit one instance identified by `request.id`, keeping its ID, kind, and
/// creation time, and return the committed record.
///
/// The edit succeeds only when the stored `revision` still equals
/// `request.expected_revision` (optimistic concurrency): a form loaded before
/// another window's save cannot overwrite that newer save. The check and the
/// write run in one immediate transaction, so another app instance cannot slip
/// a change in between.
///
/// `updated_at` uses `max(now, created_at)`, so a clock that moved backwards
/// cannot make an edit look older than the creation.
///
/// # Errors
/// `InvalidRequest` for a malformed ID or revision, `NotFound`,
/// `RevisionConflict`, `InvalidStoredProvider`, validation codes,
/// `StorageUnavailable`, `ReadFailed`, or `WriteFailed` before commit. A failed
/// commit returns `OperationFailed`, because the outcome cannot be confirmed.
pub fn update_provider(
    storage: &Storage,
    request: &UpdateProviderRequest,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    commit_provider_update(storage, request, None, now_ms).map_err(|error| match error {
        CommitFailure::NotCommitted(source) => source,
        CommitFailure::Uncertain(_) => ProviderError::OperationFailed,
    })
}

/// Save settings and optionally replace an API key through one coordinated operation.
///
/// `None` preserves the key and never accesses the credential store. A supplied key
/// is validated along with the settings before any OS prompt. The old key is kept
/// only in memory while settings and its reference commit in one SQLite transaction.
/// A concurrent settings change is rechecked after the OS call, and a definite
/// database failure restores the previous key. No database lock spans an OS prompt.
/// Callers must serialize credential writes within the app process.
///
/// # Errors
/// Returns validation, revision, storage or credential-store codes when nothing
/// changed (or was restored). Failed rollback or uncertain commit returns
/// `OperationFailed`: callers must reload and must not claim success or rollback.
/// As with other credential writes, separate app processes are not serialized.
pub fn update_provider_with_secret(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: &UpdateProviderRequest,
    secret: Option<Secret>,
    now_ms: i64,
) -> Result<ProviderRecord, ProviderError> {
    let Some(secret) = secret else {
        return update_provider(storage, request, now_ms);
    };
    let record = get_provider(storage, &request.id)?;
    if request.expected_revision < 1 {
        return Err(ProviderError::InvalidRequest);
    }
    if request.expected_revision != record.revision {
        return Err(ProviderError::RevisionConflict);
    }
    validate_settings(
        record.kind,
        &request.display_name,
        &request.base_url,
        &request.protocol,
        &request.extensions,
    )?;
    let secret = validate_secret(secret)?;
    let reference = record.id.credential_reference();
    let result = replace_then_commit(store, &reference, &secret, || {
        commit_provider_update(storage, request, Some(&reference), now_ms)
    });
    match result {
        Ok(record) => Ok(record),
        Err(ReplaceError::Credential(error)) => Err(error.into()),
        Err(ReplaceError::CommitFailed {
            source,
            restored: true,
        }) => Err(source),
        Err(ReplaceError::CommitFailed {
            restored: false, ..
        })
        | Err(ReplaceError::CommitUncertain { .. }) => Err(ProviderError::OperationFailed),
    }
}

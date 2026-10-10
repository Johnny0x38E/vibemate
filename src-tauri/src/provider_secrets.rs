//! API key status and replacement for saved provider instances.
//!
//! Every provider created since schema version 6 has its key in the OS credential
//! store under `provider-<id>` and one `provider_credential` row that records this
//! entry name. This module:
//! - reports whether a key is set using only that SQLite row, so showing a page
//!   never touches the credential store (which may show a system prompt);
//! - replaces a key, keeping the credential store and SQLite consistent.
//!
//! Rows saved before keys existed have no reference row and report `Missing`;
//! replacing then works as setting the key for the first time. There is no
//! command to clear a key: that arrives together with deleting a provider.
//! Changing a key never changes the provider's `revision`, so an open settings
//! form does not become stale.
//!
//! The key never enters SQLite, a response, an error, or `Debug` output.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::credentials::{
    CommitFailure, CredentialStore, ReplaceError, Secret, replace_then_commit, validate_secret,
};
use crate::providers::{ProviderError, ProviderId};
use crate::storage::Storage;

/// Whether a provider has a key, as recorded in SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSecretState {
    /// A reference row exists, so a key was saved in the OS credential store.
    Set,
    /// No reference row: only possible for rows saved before keys existed, or
    /// after an unknown create outcome. Replacing the key sets it.
    Missing,
}

/// Key status returned to the frontend. It never contains the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSecretStatus {
    /// The provider this status belongs to.
    pub provider_id: ProviderId,
    /// Set or missing.
    pub state: ProviderSecretState,
    /// When the key was last saved, in Unix epoch milliseconds; `None` when missing.
    pub updated_at_ms: Option<i64>,
}

/// A new key for an existing provider.
///
/// `Secret` prints as `<redacted>`, so the derived `Debug` never shows the key.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceProviderSecretRequest {
    /// The provider whose key is replaced, identified by ID only.
    pub provider_id: String,
    /// The new key; validated with `credentials::validate_secret`.
    pub secret: Secret,
}

/// Read a provider's key status from SQLite only.
///
/// # Errors
/// `InvalidRequest` for a malformed ID, `NotFound`, `StorageUnavailable`, or
/// `ReadFailed`.
pub fn get_secret_status(
    storage: &Storage,
    provider_id: &str,
) -> Result<ProviderSecretStatus, ProviderError> {
    let id = ProviderId::parse(provider_id).ok_or(ProviderError::InvalidRequest)?;
    let connection = storage
        .lock()
        .map_err(|_| ProviderError::StorageUnavailable)?;
    read_status(&connection, &id)?.ok_or(ProviderError::NotFound)
}

/// Replace (or, for a missing key, set) a provider's key and return the new status.
///
/// Steps:
/// 1. Check the ID and the key, then confirm in SQLite that the provider exists.
///    An unknown provider never reaches the credential store.
/// 2. `replace_then_commit` reads the old key as an in-memory backup and writes
///    the new one under `provider-<id>`.
/// 3. Its commit step inserts or updates the reference row in one transaction.
/// 4. If the transaction fails before `COMMIT`, the old key is written back (or the
///    new one deleted when there was none). If restoring fails too, the result is
///    `SecretOutcomeUnknown`.
/// 5. If `COMMIT` itself fails, the reference row may have been saved anyway, so
///    the new key is kept and the result is `SecretOutcomeUnknown`. Restoring here
///    could turn a first key into a `Set` row with no key behind it.
///
/// # Errors
/// `InvalidRequest`, `SecretInvalid`, `NotFound`, `StorageUnavailable`,
/// `ReadFailed`, credential-store codes (nothing changed), `WriteFailed` (the
/// previous key was restored), or `SecretOutcomeUnknown`.
pub fn replace_provider_secret(
    storage: &Storage,
    store: &dyn CredentialStore,
    request: ReplaceProviderSecretRequest,
    now_ms: i64,
) -> Result<ProviderSecretStatus, ProviderError> {
    let id = ProviderId::parse(&request.provider_id).ok_or(ProviderError::InvalidRequest)?;
    let secret = validate_secret(request.secret)?;
    {
        // Release the connection before the credential store, which may wait for
        // a system prompt.
        let connection = storage
            .lock()
            .map_err(|_| ProviderError::StorageUnavailable)?;
        read_status(&connection, &id)?.ok_or(ProviderError::NotFound)?;
    }
    let reference = id.credential_reference();
    let replaced = replace_then_commit(store, &reference, &secret, || {
        save_reference(storage, &id, &reference, now_ms)
    });
    match replaced {
        Ok(status) => Ok(status),
        Err(ReplaceError::Credential(error)) => Err(error.into()),
        // The previous key is back, so the SQLite error describes the whole outcome.
        Err(ReplaceError::CommitFailed {
            source,
            restored: true,
        }) => Err(source),
        Err(ReplaceError::CommitFailed {
            restored: false, ..
        })
        | Err(ReplaceError::CommitUncertain { .. }) => Err(ProviderError::SecretOutcomeUnknown),
    }
}

/// Read the status, or `None` when no provider has this ID.
///
/// The LEFT JOIN keeps a provider without a reference row, which becomes `Missing`.
fn read_status(
    connection: &Connection,
    id: &ProviderId,
) -> Result<Option<ProviderSecretStatus>, ProviderError> {
    let updated_at: Option<Option<i64>> = connection
        .query_row(
            "SELECT credential.updated_at
             FROM provider_instance AS provider
             LEFT JOIN provider_credential AS credential
                 ON credential.provider_id = provider.id
             WHERE provider.id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ProviderError::ReadFailed)?;
    Ok(updated_at.map(|updated_at| ProviderSecretStatus {
        provider_id: id.clone(),
        state: if updated_at.is_some() {
            ProviderSecretState::Set
        } else {
            ProviderSecretState::Missing
        },
        updated_at_ms: updated_at,
    }))
}

/// Insert or update the reference row for `id` in one transaction.
///
/// The provider is checked again inside the transaction: it could have been
/// removed while the credential store was busy. Only `provider_credential`
/// changes, so the provider's `revision` and `updated_at` stay as they were.
///
/// Failures before `COMMIT` are `NotCommitted`; a failed `COMMIT` is `Uncertain`.
fn save_reference(
    storage: &Storage,
    id: &ProviderId,
    reference: &str,
    now_ms: i64,
) -> Result<ProviderSecretStatus, CommitFailure<ProviderError>> {
    let not_committed = CommitFailure::NotCommitted;
    let mut connection = storage
        .lock()
        .map_err(|_| not_committed(ProviderError::StorageUnavailable))?;
    // Dropping a transaction without `commit` rolls it back.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| not_committed(ProviderError::WriteFailed))?;
    let exists = transaction
        .query_row(
            "SELECT 1 FROM provider_instance WHERE id = ?1",
            [id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map_err(|_| not_committed(ProviderError::ReadFailed))?;
    if exists.is_none() {
        return Err(not_committed(ProviderError::NotFound));
    }
    transaction
        .execute(
            "INSERT INTO provider_credential (provider_id, credential_ref, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (provider_id) DO UPDATE SET updated_at = excluded.updated_at",
            params![id.as_str(), reference, now_ms],
        )
        .map_err(|_| not_committed(ProviderError::WriteFailed))?;
    transaction
        .commit()
        .map_err(|_| CommitFailure::Uncertain(ProviderError::WriteFailed))?;
    Ok(ProviderSecretStatus {
        provider_id: id.clone(),
        state: ProviderSecretState::Set,
        updated_at_ms: Some(now_ms),
    })
}

#[cfg(test)]
mod tests;

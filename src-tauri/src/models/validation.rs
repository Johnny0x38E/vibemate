//! Encode and decode bounded model-list cursors.

use super::ModelError;
use crate::model_identity::is_valid_model_id;

/// Longest cursor: a 256-character model ID is at most 1024 UTF-8 bytes.
pub(super) const MAX_CURSOR_LEN: usize = 2048;

/// Encode a model ID as a cursor: lowercase hex of its UTF-8 bytes.
pub fn encode_cursor(model_id: &str) -> String {
    model_id.bytes().map(|byte| format!("{byte:02x}")).collect()
}

/// Decode a cursor made by `encode_cursor`. Anything else is `InvalidRequest`.
pub fn decode_cursor(cursor: &str) -> Result<String, ModelError> {
    let well_formed = !cursor.is_empty()
        && cursor.len() <= MAX_CURSOR_LEN
        && cursor.len().is_multiple_of(2)
        && cursor
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !well_formed {
        return Err(ModelError::InvalidRequest);
    }
    let bytes = (0..cursor.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&cursor[index..index + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| ModelError::InvalidRequest)?;
    let model_id = String::from_utf8(bytes).map_err(|_| ModelError::InvalidRequest)?;
    if is_valid_model_id(&model_id) {
        Ok(model_id)
    } else {
        Err(ModelError::InvalidRequest)
    }
}

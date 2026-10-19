//! Bounded read of Pi's most recent session metadata for the active model.
//!
//! Pi's `/model` switch is session-scoped: it updates the session JSONL but
//! usually not `settings.json`, whose `defaultModel`/`defaultProvider` only
//! describe the startup default. To show the model the Agent actually uses,
//! this module finds the most recently modified session file below
//! `<agent-dir>/sessions/`, reads at most the last [`TAIL_BYTES`], and extracts
//! only the provider and model from a `model_change` event or an assistant
//! message. Conversation text, tool output and every other field are discarded
//! immediately. Nothing here crosses IPC or enters a log.
//!
//! The result is the latest recorded model, not proof of a currently running
//! process. Multiple Pi sessions can exist; the newest file is the best
//! available observation and is documented as such.

use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Bytes read from the end of one session file.
const TAIL_BYTES: u64 = 512 * 1024;

/// Most session files inspected when looking for the newest one.
const MAX_SESSION_FILES: usize = 10_000;

/// Longest provider key and model id accepted from session metadata.
const MAX_PROVIDER_CHARS: usize = 160;
const MAX_MODEL_CHARS: usize = 256;

/// The last provider/model pair recorded in Pi's newest session.
///
/// Returns `None` when the sessions directory is missing, no regular `.jsonl`
/// session can be read, or the tail carries no usable model metadata.
pub(super) fn latest_model(root: &Path) -> Option<(String, String)> {
    let path = newest_session(&root.join("sessions"))?;
    let (tail, truncated) = read_tail(&path, TAIL_BYTES)?;
    let text = String::from_utf8_lossy(&tail);
    let mut lines = text.split('\n');
    // A truncated tail starts inside a line; that prefix is not valid JSON.
    if truncated {
        lines.next();
    }
    let mut latest = None;
    for line in lines {
        if let Some(pair) = model_from_line(line) {
            latest = Some(pair);
        }
    }
    latest
}

/// The most recently modified direct `sessions/<project>/<file>.jsonl` file.
///
/// Directory entries are inspected without following symbolic links. A missing
/// or unreadable sessions directory is not an error; the caller falls back to
/// the startup defaults.
fn newest_session(sessions: &Path) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    let mut inspected = 0usize;
    let projects = fs::read_dir(sessions).ok()?;
    for project in projects {
        let Ok(project) = project else {
            continue;
        };
        let Ok(file_type) = project.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(project.path()) else {
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl")
            {
                continue;
            }
            inspected += 1;
            if inspected > MAX_SESSION_FILES {
                break;
            }
            let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
                continue;
            };
            if newest
                .as_ref()
                .is_none_or(|(current, _)| modified > *current)
            {
                newest = Some((modified, entry.path()));
            }
        }
    }
    newest.map(|(_, path)| path)
}

/// Read the end of one regular session file.
fn read_tail(path: &Path, limit: u64) -> Option<(Vec<u8>, bool)> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() {
        return None;
    }
    let length = metadata.len();
    let start = length.saturating_sub(limit);
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity(usize::try_from(length - start).ok()?);
    file.take(limit).read_to_end(&mut bytes).ok()?;
    Some((bytes, start > 0))
}

/// Extract a provider/model pair from one session line, ignoring everything else.
fn model_from_line(line: &str) -> Option<(String, String)> {
    if !line.contains("model_change") && !line.contains("\"assistant\"") {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(Value::as_str) == Some("model_change") {
        let provider = value.get("provider").and_then(Value::as_str)?;
        let model = value.get("modelId").and_then(Value::as_str)?;
        return safe_pair(provider, model);
    }
    let message = value.get("message")?;
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let provider = message.get("provider").and_then(Value::as_str)?;
    let model = message.get("model").and_then(Value::as_str)?;
    safe_pair(provider, model)
}

/// Accept only bounded, printable identity text.
fn safe_pair(provider: &str, model: &str) -> Option<(String, String)> {
    let safe = |value: &str, limit: usize| {
        !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
    };
    (safe(provider, MAX_PROVIDER_CHARS) && safe(model, MAX_MODEL_CHARS))
        .then(|| (provider.to_owned(), model.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "vibemate-pi-session-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("sessions/project")).unwrap();
            Self(root)
        }

        fn write(&self, name: &str, text: &str) {
            fs::write(self.0.join("sessions/project").join(name), text).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_last_model_change_or_assistant_message_wins() {
        let fixture = Fixture::new();
        fixture.write(
            "session.jsonl",
            &format!(
                "{}\n{}\n{}\n",
                r#"{"type":"model_change","provider":"old","modelId":"old-model"}"#,
                r#"{"type":"message","message":{"role":"assistant","provider":"new","model":"new-model"}}"#,
                r#"{"type":"model_change","provider":"latest","modelId":"latest-model"}"#
            ),
        );
        assert_eq!(
            latest_model(&fixture.0),
            Some(("latest".into(), "latest-model".into()))
        );
    }

    #[test]
    fn assistant_metadata_is_used_when_the_tail_has_no_model_change() {
        let fixture = Fixture::new();
        fixture.write(
            "session.jsonl",
            concat!(
                "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"ignored\"}}\n",
                "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",",
                "\"provider\":\"command-code\",\"model\":\"gpt-6-luna\"}}\n"
            ),
        );
        assert_eq!(
            latest_model(&fixture.0),
            Some(("command-code".into(), "gpt-6-luna".into()))
        );
    }

    #[test]
    fn unreadable_or_absent_sessions_return_none() {
        let fixture = Fixture::new();
        assert_eq!(latest_model(&fixture.0), None);
        fs::remove_dir_all(fixture.0.join("sessions")).unwrap();
        assert_eq!(latest_model(&fixture.0), None);
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_not_followed() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let outside = fixture.0.join("outside.jsonl");
        fs::write(
            &outside,
            "{\"type\":\"model_change\",\"provider\":\"p\",\"modelId\":\"m\"}\n",
        )
        .unwrap();
        symlink(&outside, fixture.0.join("sessions/project/linked.jsonl")).unwrap();
        assert_eq!(latest_model(&fixture.0), None);
    }
}

//! Validated appearance and color-theme preferences in private app storage.
//! No Tauri, OS appearance detection, or frontend styling belongs in this module.

use crate::settings::SettingsError;
use crate::storage::Storage;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

/// Brightness policy; system mode leaves OS changes to frontend CSS.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    /// Follow the system color scheme.
    #[default]
    System,
    /// Always use the light palette.
    Light,
    /// Always use the dark palette.
    Dark,
}

impl Appearance {
    fn stored_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
    fn parse(value: &str) -> Result<Self, SettingsError> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(SettingsError::InvalidPreference),
        }
    }
}

/// Stable built-in palette identity, independent of brightness policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Calm green palette matching the product identity.
    #[default]
    Forest,
    /// Neutral monochrome palette.
    Graphite,
    /// Warm paper and clay palette.
    Linen,
    /// Restrained violet accents.
    Iris,
    /// Blue accents on cool neutral surfaces.
    Ocean,
    /// White and warm-gray surfaces inspired by Notion.
    Notion,
}

impl Theme {
    fn stored_value(self) -> &'static str {
        match self {
            Self::Forest => "forest",
            Self::Graphite => "graphite",
            Self::Linen => "linen",
            Self::Iris => "iris",
            Self::Ocean => "ocean",
            Self::Notion => "notion",
        }
    }
    fn parse(value: &str) -> Result<Self, SettingsError> {
        match value {
            "forest" => Ok(Self::Forest),
            "graphite" => Ok(Self::Graphite),
            "linen" => Ok(Self::Linen),
            "iris" => Ok(Self::Iris),
            "ocean" => Ok(Self::Ocean),
            "notion" => Ok(Self::Notion),
            _ => Err(SettingsError::InvalidPreference),
        }
    }
}

/// Appearance and palette saved together so their confirmed state cannot split.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearancePreference {
    /// Whether brightness follows the OS or an explicit light/dark choice.
    pub appearance: Appearance,
    /// Which pair of light/dark palettes to use.
    pub theme: Theme,
}

/// Read a validated pair. A missing row means system/forest without writing defaults.
///
/// # Errors
/// Returns safe storage/read errors or `InvalidPreference` for corrupt values;
/// invalid data is never silently replaced.
pub fn load_appearance_preference(
    storage: &Storage,
) -> Result<AppearancePreference, SettingsError> {
    let connection = storage
        .lock()
        .map_err(|_| SettingsError::StorageUnavailable)?;
    let stored: Option<(String, String)> = connection
        .query_row(
            "SELECT appearance, theme FROM appearance_preference WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| SettingsError::ReadFailed)?;
    match stored {
        Some((appearance, theme)) => Ok(AppearancePreference {
            appearance: Appearance::parse(&appearance)?,
            theme: Theme::parse(&theme)?,
        }),
        None => Ok(AppearancePreference::default()),
    }
}

/// Atomically save the validated pair and return it only after SQLite commits.
/// Language preferences and all unrelated rows are untouched.
///
/// # Errors
/// Returns `StorageUnavailable` or `WriteFailed`; a failed write preserves the
/// previously committed pair.
pub fn save_appearance_preference(
    storage: &Storage,
    preference: AppearancePreference,
) -> Result<AppearancePreference, SettingsError> {
    let connection = storage
        .lock()
        .map_err(|_| SettingsError::StorageUnavailable)?;
    connection.execute("INSERT INTO appearance_preference (id, appearance, theme) VALUES (1, ?1, ?2) ON CONFLICT(id) DO UPDATE SET appearance = excluded.appearance, theme = excluded.theme", [preference.appearance.stored_value(), preference.theme.stored_value()]).map_err(|_| SettingsError::WriteFailed)?;
    Ok(preference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "vibemate-appearance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn appearance_defaults_without_saving_and_reopens_saved_pair() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open");
        assert_eq!(
            load_appearance_preference(&storage),
            Ok(AppearancePreference::default())
        );
        let count: u32 = storage
            .lock()
            .expect("lock")
            .query_row("SELECT COUNT(*) FROM appearance_preference", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, 0);
        let choice = AppearancePreference {
            appearance: Appearance::Dark,
            theme: Theme::Iris,
        };
        assert_eq!(save_appearance_preference(&storage, choice), Ok(choice));
        drop(storage);
        let storage = Storage::open_in_directory(&directory.0).expect("reopen");
        assert_eq!(load_appearance_preference(&storage), Ok(choice));
    }
    #[test]
    fn appearance_all_themes_and_brightness_choices_round_trip() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open");
        for theme in [
            Theme::Forest,
            Theme::Graphite,
            Theme::Linen,
            Theme::Iris,
            Theme::Ocean,
            Theme::Notion,
        ] {
            for appearance in [Appearance::System, Appearance::Light, Appearance::Dark] {
                let choice = AppearancePreference { appearance, theme };
                assert_eq!(save_appearance_preference(&storage, choice), Ok(choice));
                assert_eq!(load_appearance_preference(&storage), Ok(choice));
            }
        }
    }

    #[test]
    fn appearance_migration_preserves_v2_locale_and_unrelated_data() {
        let directory = TestDirectory::new();
        let connection =
            rusqlite::Connection::open(directory.0.join("vibemate.sqlite3")).expect("open v2");
        connection.execute_batch("PRAGMA user_version = 2; CREATE TABLE locale_preference (id INTEGER PRIMARY KEY, preference TEXT); INSERT INTO locale_preference VALUES (1, 'en'); CREATE TABLE sample (value TEXT); INSERT INTO sample VALUES ('kept');").expect("v2 fixture");
        drop(connection);
        let storage = Storage::open_in_directory(&directory.0).expect("upgrade");
        assert_eq!(storage.schema_version().expect("version"), 4);
        assert_eq!(
            crate::settings::load_locale_preference(&storage),
            Ok(crate::settings::LocalePreference::English)
        );
        assert_eq!(
            load_appearance_preference(&storage),
            Ok(AppearancePreference::default())
        );
        let value: String = storage
            .lock()
            .expect("lock")
            .query_row("SELECT value FROM sample", [], |row| row.get(0))
            .expect("original data");
        assert_eq!(value, "kept");
    }

    #[test]
    fn appearance_failed_save_preserves_committed_pair() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open");
        let previous = AppearancePreference {
            appearance: Appearance::Dark,
            theme: Theme::Linen,
        };
        save_appearance_preference(&storage, previous).expect("save");
        storage
            .lock()
            .expect("lock")
            .pragma_update(None, "query_only", true)
            .expect("read only");
        assert_eq!(
            save_appearance_preference(&storage, AppearancePreference::default()),
            Err(SettingsError::WriteFailed)
        );
        assert_eq!(load_appearance_preference(&storage), Ok(previous));
    }

    #[test]
    fn appearance_corrupt_values_are_reported_and_not_replaced() {
        let directory = TestDirectory::new();
        let storage = Storage::open_in_directory(&directory.0).expect("open");
        storage.lock().expect("lock").execute_batch("PRAGMA ignore_check_constraints = ON; INSERT INTO appearance_preference VALUES (1, 'dark', 'unknown');").expect("corrupt fixture");
        assert_eq!(
            load_appearance_preference(&storage),
            Err(SettingsError::InvalidPreference)
        );
        let value: String = storage
            .lock()
            .expect("lock")
            .query_row("SELECT theme FROM appearance_preference", [], |row| {
                row.get(0)
            })
            .expect("stored value");
        assert_eq!(value, "unknown");
        storage
            .lock()
            .expect("lock")
            .execute_batch("DROP TABLE appearance_preference")
            .expect("broken schema");
        assert_eq!(
            load_appearance_preference(&storage),
            Err(SettingsError::ReadFailed)
        );
    }
}

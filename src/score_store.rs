use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::difficulty::{DifficultyPreset, HighScores, PlayerProfile};

pub struct ScoreStore {
    data_dir: Option<PathBuf>,
    saved_profile: PlayerProfile,
    last_attempted_profile: PlayerProfile,
    migration_pending: bool,
}

impl ScoreStore {
    pub fn discover() -> Self {
        Self::from_optional_dir(default_data_dir())
    }

    fn from_optional_dir(data_dir: Option<PathBuf>) -> Self {
        Self {
            data_dir,
            saved_profile: PlayerProfile::default(),
            last_attempted_profile: PlayerProfile::default(),
            migration_pending: false,
        }
    }

    #[cfg(test)]
    fn from_dir(data_dir: PathBuf) -> Self {
        Self::from_optional_dir(Some(data_dir))
    }

    /// Missing, unreadable, or corrupt files fall back to a Normal profile so
    /// local state can never prevent the game from starting. A legacy single
    /// `high_score` integer is imported as the Normal record.
    pub fn load(&mut self) -> PlayerProfile {
        let Some(data_dir) = self.data_dir.as_deref() else {
            return PlayerProfile::default();
        };

        let selected_difficulty = fs::read_to_string(data_dir.join("settings"))
            .ok()
            .and_then(|contents| parse_selected_difficulty(&contents))
            .unwrap_or_default();

        let scores_path = data_dir.join("high_scores");
        let scores_contents = fs::read_to_string(&scores_path).ok();
        let (high_scores, migrated_legacy) = match scores_contents {
            Some(contents) => (parse_high_scores(&contents), false),
            None => {
                let legacy_score = fs::read_to_string(data_dir.join("high_score"))
                    .ok()
                    .and_then(|contents| contents.trim().parse::<u32>().ok());
                let mut scores = HighScores::default();
                if let Some(score) = legacy_score {
                    scores.set(DifficultyPreset::Normal, score);
                }
                (scores, legacy_score.is_some())
            }
        };

        let profile = PlayerProfile {
            selected_difficulty,
            high_scores,
        };
        self.saved_profile = profile;
        self.last_attempted_profile = profile;
        self.migration_pending = migrated_legacy;
        profile
    }

    /// Persist settings and per-difficulty records only when they change. Each
    /// file is written through a temp file before rename so partial contents do
    /// not replace the last valid local state.
    pub fn save_if_changed(&mut self, profile: PlayerProfile) -> io::Result<bool> {
        if !self.migration_pending
            && (profile == self.saved_profile || profile == self.last_attempted_profile)
        {
            return Ok(false);
        }
        self.last_attempted_profile = profile;

        let Some(data_dir) = self.data_dir.as_deref() else {
            return Ok(false);
        };
        self.migration_pending = true;
        fs::create_dir_all(data_dir)?;

        write_atomic(
            &data_dir.join("settings"),
            &format!("difficulty={}\n", profile.selected_difficulty.storage_key()),
        )?;
        write_atomic(
            &data_dir.join("high_scores"),
            &format!(
                "easy={}\nnormal={}\nhard={}\nextreme={}\n",
                profile.high_scores.get(DifficultyPreset::Easy),
                profile.high_scores.get(DifficultyPreset::Normal),
                profile.high_scores.get(DifficultyPreset::Hard),
                profile.high_scores.get(DifficultyPreset::Extreme),
            ),
        )?;

        self.saved_profile = profile;
        self.migration_pending = false;
        Ok(true)
    }
}

fn parse_selected_difficulty(contents: &str) -> Option<DifficultyPreset> {
    let value = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("difficulty="))?;
    DifficultyPreset::from_storage_key(value)
}

fn parse_high_scores(contents: &str) -> HighScores {
    let mut scores = HighScores::default();
    for line in contents.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let (Some(preset), Ok(score)) = (
            DifficultyPreset::from_storage_key(key),
            value.trim().parse::<u32>(),
        ) else {
            continue;
        };
        scores.set(preset, score);
    }
    scores
}

fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, contents)?;
    replace_file(&temp_path, path)
}

fn default_data_dir() -> Option<PathBuf> {
    if let Some(override_dir) = env::var_os("SKYSTRIKE_DATA_DIR") {
        return Some(PathBuf::from(override_dir));
    }

    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("skystrike")
        });
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
            return Some(PathBuf::from(data_home).join("skystrike"));
        }
        return env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share/skystrike"));
    }

    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("skystrike"));
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(not(target_os = "windows"))]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    fs::rename(temp_path, path)
}

#[cfg(target_os = "windows")]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn test_dir(name: &str) -> PathBuf {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!("skystrike-{name}-{}-{id}", std::process::id()))
    }

    #[test]
    fn missing_or_corrupt_profile_uses_normal_defaults() {
        let dir = test_dir("corrupt-profile");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings"), "difficulty=impossible\n").unwrap();
        fs::write(dir.join("high_scores"), "easy=nope\nunknown=900\n").unwrap();

        let mut store = ScoreStore::from_dir(dir.clone());
        assert_eq!(store.load(), PlayerProfile::default());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn profile_round_trips_all_scores_and_selected_difficulty() {
        let dir = test_dir("round-trip");
        let mut scores = HighScores::default();
        scores.set(DifficultyPreset::Easy, 100);
        scores.set(DifficultyPreset::Normal, 200);
        scores.set(DifficultyPreset::Hard, 300);
        scores.set(DifficultyPreset::Extreme, 400);
        let profile = PlayerProfile {
            selected_difficulty: DifficultyPreset::Hard,
            high_scores: scores,
        };
        let mut store = ScoreStore::from_dir(dir.clone());

        assert!(store.save_if_changed(profile).unwrap());
        assert!(!store.save_if_changed(profile).unwrap());
        assert_eq!(
            fs::read_to_string(dir.join("settings")).unwrap(),
            "difficulty=hard\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("high_scores")).unwrap(),
            "easy=100\nnormal=200\nhard=300\nextreme=400\n"
        );

        let mut reloaded = ScoreStore::from_dir(dir.clone());
        assert_eq!(reloaded.load(), profile);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_single_score_migrates_to_normal() {
        let dir = test_dir("legacy");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("high_score"), "4321\n").unwrap();
        let mut store = ScoreStore::from_dir(dir.clone());

        let profile = store.load();
        assert_eq!(profile.selected_difficulty, DifficultyPreset::Normal);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Easy), 0);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Normal), 4321);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Hard), 0);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Extreme), 0);
        assert!(store.save_if_changed(profile).unwrap());
        assert!(dir.join("high_scores").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn three_difficulty_score_file_defaults_extreme_to_zero() {
        let dir = test_dir("three-difficulty");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("high_scores"), "easy=100\nnormal=200\nhard=300\n").unwrap();
        let mut store = ScoreStore::from_dir(dir.clone());

        let profile = store.load();
        assert_eq!(profile.high_scores.get(DifficultyPreset::Easy), 100);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Normal), 200);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Hard), 300);
        assert_eq!(profile.high_scores.get(DifficultyPreset::Extreme), 0);

        let _ = fs::remove_dir_all(dir);
    }
}

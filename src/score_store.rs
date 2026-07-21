use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct ScoreStore {
    path: Option<PathBuf>,
    saved_score: u32,
    last_attempted_score: u32,
}

impl ScoreStore {
    pub fn discover() -> Self {
        Self::from_optional_path(default_score_path())
    }

    fn from_optional_path(path: Option<PathBuf>) -> Self {
        Self {
            path,
            saved_score: 0,
            last_attempted_score: 0,
        }
    }

    #[cfg(test)]
    fn from_path(path: PathBuf) -> Self {
        Self::from_optional_path(Some(path))
    }

    /// Missing/unreadable/corrupt state is treated as no high score so a
    /// local data problem can never prevent the game from starting.
    pub fn load(&mut self) -> u32 {
        let score = self
            .path
            .as_deref()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|contents| contents.trim().parse::<u32>().ok())
            .unwrap_or(0);
        self.saved_score = score;
        self.last_attempted_score = score;
        score
    }

    /// Persist only a new record. The temp-file + rename sequence prevents a
    /// partial integer from replacing a previously valid score.
    pub fn save_if_higher(&mut self, score: u32) -> io::Result<bool> {
        if score <= self.saved_score || score <= self.last_attempted_score {
            return Ok(false);
        }
        self.last_attempted_score = score;

        let Some(path) = self.path.as_deref() else {
            return Ok(false);
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, format!("{score}\n"))?;
        replace_file(&temp_path, path)?;
        self.saved_score = score;
        Ok(true)
    }
}

fn default_score_path() -> Option<PathBuf> {
    if let Some(override_dir) = env::var_os("SKYSTRIKE_DATA_DIR") {
        return Some(PathBuf::from(override_dir).join("high_score"));
    }

    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("skystrike")
                .join("high_score")
        });
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
            return Some(PathBuf::from(data_home).join("skystrike/high_score"));
        }
        return env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share/skystrike/high_score"));
    }

    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("skystrike").join("high_score"));
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

    fn test_path(name: &str) -> PathBuf {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!("skystrike-{name}-{}-{id}", std::process::id()))
    }

    #[test]
    fn missing_or_corrupt_scores_load_as_zero() {
        let path = test_path("corrupt");
        let mut store = ScoreStore::from_path(path.clone());
        assert_eq!(store.load(), 0);

        fs::write(&path, "not-a-number").unwrap();
        assert_eq!(store.load(), 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn only_higher_scores_are_persisted() {
        let path = test_path("higher");
        let mut store = ScoreStore::from_path(path.clone());

        assert!(store.save_if_higher(120).unwrap());
        assert!(!store.save_if_higher(100).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "120\n");
        assert!(!path.with_extension("tmp").exists());

        let mut reloaded = ScoreStore::from_path(path.clone());
        assert_eq!(reloaded.load(), 120);
        let _ = fs::remove_file(path);
    }
}

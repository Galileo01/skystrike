#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DifficultyPreset {
    Easy,
    #[default]
    Normal,
    Hard,
    Extreme,
}

impl DifficultyPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "EASY",
            Self::Normal => "NORMAL",
            Self::Hard => "HARD",
            Self::Extreme => "EXTREME",
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
            Self::Extreme => "extreme",
        }
    }

    pub fn from_storage_key(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "easy" => Some(Self::Easy),
            "normal" => Some(Self::Normal),
            "hard" => Some(Self::Hard),
            "extreme" => Some(Self::Extreme),
            _ => None,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Easy => Self::Extreme,
            Self::Normal => Self::Easy,
            Self::Hard => Self::Normal,
            Self::Extreme => Self::Hard,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Easy => Self::Normal,
            Self::Normal => Self::Hard,
            Self::Hard => Self::Extreme,
            Self::Extreme => Self::Easy,
        }
    }

    pub fn spawn_interval_multiplier(self) -> f32 {
        match self {
            Self::Easy => 1.25,
            Self::Normal => 1.0,
            Self::Hard => 0.8,
            Self::Extreme => 0.65,
        }
    }

    pub fn enemy_speed_multiplier(self) -> f32 {
        match self {
            Self::Easy => 0.85,
            Self::Normal => 1.0,
            Self::Hard => 1.15,
            Self::Extreme => 1.30,
        }
    }

    pub fn targeted_spawn_probability(self) -> f64 {
        match self {
            Self::Easy => 0.0,
            Self::Normal => 0.10,
            Self::Hard => 0.30,
            Self::Extreme => 0.50,
        }
    }

    pub fn targeted_spawn_radius(self) -> i32 {
        match self {
            Self::Easy => 0,
            Self::Normal => 16,
            Self::Hard => 12,
            Self::Extreme => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HighScores {
    easy: u32,
    normal: u32,
    hard: u32,
    extreme: u32,
}

impl HighScores {
    pub fn get(self, preset: DifficultyPreset) -> u32 {
        match preset {
            DifficultyPreset::Easy => self.easy,
            DifficultyPreset::Normal => self.normal,
            DifficultyPreset::Hard => self.hard,
            DifficultyPreset::Extreme => self.extreme,
        }
    }

    pub fn set(&mut self, preset: DifficultyPreset, score: u32) {
        match preset {
            DifficultyPreset::Easy => self.easy = score,
            DifficultyPreset::Normal => self.normal = score,
            DifficultyPreset::Hard => self.hard = score,
            DifficultyPreset::Extreme => self.extreme = score,
        }
    }

    pub fn update_if_higher(&mut self, preset: DifficultyPreset, score: u32) -> bool {
        if score <= self.get(preset) {
            return false;
        }
        self.set(preset, score);
        true
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerProfile {
    pub selected_difficulty: DifficultyPreset,
    pub high_scores: HighScores,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_is_the_default_and_presets_cycle() {
        assert_eq!(DifficultyPreset::default(), DifficultyPreset::Normal);
        assert_eq!(DifficultyPreset::Normal.previous(), DifficultyPreset::Easy);
        assert_eq!(DifficultyPreset::Normal.next(), DifficultyPreset::Hard);
        assert_eq!(DifficultyPreset::Easy.previous(), DifficultyPreset::Extreme);
        assert_eq!(DifficultyPreset::Hard.next(), DifficultyPreset::Extreme);
        assert_eq!(DifficultyPreset::Extreme.next(), DifficultyPreset::Easy);
    }

    #[test]
    fn high_scores_are_independent_per_difficulty() {
        let mut scores = HighScores::default();
        scores.set(DifficultyPreset::Easy, 100);
        scores.set(DifficultyPreset::Normal, 200);
        scores.set(DifficultyPreset::Hard, 300);
        scores.set(DifficultyPreset::Extreme, 400);

        assert_eq!(scores.get(DifficultyPreset::Easy), 100);
        assert_eq!(scores.get(DifficultyPreset::Normal), 200);
        assert_eq!(scores.get(DifficultyPreset::Hard), 300);
        assert_eq!(scores.get(DifficultyPreset::Extreme), 400);
        assert!(!scores.update_if_higher(DifficultyPreset::Hard, 299));
    }

    #[test]
    fn higher_difficulties_target_the_player_x_more_often_and_tightly() {
        assert_eq!(DifficultyPreset::Easy.targeted_spawn_probability(), 0.0);
        assert_eq!(DifficultyPreset::Normal.targeted_spawn_probability(), 0.10);
        assert_eq!(DifficultyPreset::Hard.targeted_spawn_probability(), 0.30);
        assert_eq!(DifficultyPreset::Extreme.targeted_spawn_probability(), 0.50);
        assert!(
            DifficultyPreset::Extreme.targeted_spawn_radius()
                < DifficultyPreset::Hard.targeted_spawn_radius()
        );
    }
}

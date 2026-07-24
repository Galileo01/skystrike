use crossterm::style::Color;
use rand::{Rng, RngExt};

use crate::difficulty::DifficultyPreset;
use crate::pickup::{PickupKind, random_drop};

pub enum ObstacleType {
    Big,   // slow, wider - heavy bomber
    Small, // fast, narrow - fighter
}

impl ObstacleType {
    pub fn base_score(&self) -> u32 {
        match self {
            Self::Big => 100,
            Self::Small => 50,
        }
    }
}

pub struct Obstacle {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub width: u16,
    pub height: u16,
    pub active: bool,
    pub kind: ObstacleType,
    pub carried_pickup: Option<PickupKind>,
}

// Heavy bomber (facing down, coming from top)
const BIG_WIDTH: u16 = 11;
const BIG_SPRITE: &[&str] = &[
    "   \\_|_/   ",
    "   /*|*\\   ",
    "  /**|**\\  ",
    " /***|***\\ ",
    "/****|****\\",
    "  \\**|**/  ",
];

// Small fighter (facing down, coming from top)
const SMALL_WIDTH: u16 = 5;
const SMALL_SPRITE: &[&str] = &[" \\|/ ", "/*|*\\", " *|* ", "  *  "];
const SPAWN_HORIZONTAL_GAP: u16 = 3;
const RANDOM_SPAWN_ATTEMPTS: usize = 6;

fn big_char_color(ch: char) -> Color {
    match ch {
        '*' => Color::DarkRed,
        '|' => Color::White,
        '/' | '\\' => Color::Red,
        '_' => Color::Red,
        _ => Color::Reset,
    }
}

fn small_char_color(ch: char) -> Color {
    match ch {
        '*' => Color::DarkYellow,
        '|' => Color::White,
        '/' | '\\' => Color::Yellow,
        _ => Color::Reset,
    }
}

impl Obstacle {
    #[cfg(test)]
    pub fn new(kind: ObstacleType, x: f32) -> Self {
        Self::with_pickup(kind, x, None)
    }

    fn with_pickup(kind: ObstacleType, x: f32, carried_pickup: Option<PickupKind>) -> Self {
        let (width, height, speed) = match kind {
            ObstacleType::Big => (BIG_WIDTH, BIG_SPRITE.len() as u16, 0.35),
            ObstacleType::Small => (SMALL_WIDTH, SMALL_SPRITE.len() as u16, 0.7),
        };
        Self {
            x,
            y: -(height as f32),
            speed,
            width,
            height,
            active: true,
            kind,
            carried_pickup,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.y += self.speed * dt;
    }

    pub fn is_off_screen(&self, terminal_height: u16) -> bool {
        self.y > terminal_height as f32
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer, debug_enabled: bool) {
        if !self.active {
            return;
        }
        let (sprite, color_fn) = match self.kind {
            ObstacleType::Big => (BIG_SPRITE, big_char_color as fn(char) -> Color),
            ObstacleType::Small => (SMALL_SPRITE, small_char_color as fn(char) -> Color),
        };
        let x = self.x as u16;
        let y = self.y as u16;
        for (row, line) in sprite.iter().enumerate() {
            let cy = y + row as u16;
            for (col, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    let color = color_fn(ch);
                    renderer.put_char(x + col as u16, cy, ch, color);
                }
            }
        }

        if debug_enabled && self.y >= 3.0 {
            let (label, color) = match self.carried_pickup {
                Some(kind) => (kind.symbol(), kind.color()),
                None => ("[-]", Color::DarkGrey),
            };
            let center_x = x.saturating_add(self.width / 2);
            let label_x = center_x.saturating_sub(label.len() as u16 / 2);
            renderer.put_str(label_x, y.saturating_sub(1), label, color);
        }
    }

    pub fn hitbox(&self) -> (u16, u16, u16, u16) {
        (self.x as u16, self.y as u16, self.width, self.height)
    }
}

pub struct ObstaclePool {
    pub obstacles: Vec<Obstacle>,
    spawn_timer: f32,
    spawn_interval: f32,
    spawn_interval_multiplier: f32,
    terminal_width: u16,
}

pub struct ObstacleUpdateContext {
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub speed_multiplier: f32,
    pub difficulty: f32,
    pub player_center_x: f32,
    pub difficulty_preset: DifficultyPreset,
    pub dt: f32,
}

impl ObstaclePool {
    pub fn new(terminal_width: u16) -> Self {
        Self {
            obstacles: Vec::new(),
            spawn_timer: 0.0,
            spawn_interval: 40.0, // frame-time units between spawns (at 30 FPS baseline)
            spawn_interval_multiplier: 1.0,
            terminal_width,
        }
    }

    pub fn update(&mut self, context: ObstacleUpdateContext, rng: &mut impl Rng) {
        let ObstacleUpdateContext {
            terminal_width,
            terminal_height,
            speed_multiplier,
            difficulty,
            player_center_x,
            difficulty_preset,
            dt,
        } = context;
        self.terminal_width = terminal_width;

        // Spawn timer (frame-time units relative to the 30 FPS baseline)
        self.spawn_timer += dt;
        let interval = self.effective_spawn_interval(difficulty);
        if self.spawn_timer >= interval {
            self.spawn_timer = 0.0;
            self.spawn(player_center_x, difficulty_preset, rng);
        }

        // Update active obstacles
        for obs in &mut self.obstacles {
            if obs.active {
                obs.update(dt * speed_multiplier);
                if obs.is_off_screen(terminal_height) {
                    obs.active = false;
                }
            }
        }
    }

    fn spawn(
        &mut self,
        player_center_x: f32,
        difficulty_preset: DifficultyPreset,
        rng: &mut impl Rng,
    ) {
        let kind = if rng.random_bool(0.3) {
            ObstacleType::Big
        } else {
            ObstacleType::Small
        };
        let width = match kind {
            ObstacleType::Big => BIG_WIDTH,
            ObstacleType::Small => SMALL_WIDTH,
        };
        let x = self.choose_spawn_x(player_center_x, width, difficulty_preset, rng) as f32;
        let carried_pickup = random_drop(rng);

        // Find an inactive slot or create new after position selection, which
        // needs an immutable view of active enemies near the top.
        let slot = self.obstacles.iter_mut().find(|o| !o.active);
        match slot {
            Some(obs) => {
                *obs = Obstacle::with_pickup(kind, x, carried_pickup);
            }
            None => {
                self.obstacles
                    .push(Obstacle::with_pickup(kind, x, carried_pickup));
            }
        }
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer, debug_enabled: bool) {
        for obs in &self.obstacles {
            obs.render(renderer, debug_enabled);
        }
    }

    pub fn clear(&mut self) {
        self.obstacles.clear();
        self.spawn_timer = 0.0;
        self.spawn_interval_multiplier = 1.0;
    }

    /// Remove the current wave without scoring while preserving allocated
    /// slots for later reuse. Resetting the timer also delays the next spawn.
    pub fn clear_active_enemies(&mut self) {
        for obstacle in &mut self.obstacles {
            obstacle.active = false;
        }
        self.spawn_timer = 0.0;
    }

    pub fn set_spawn_interval_multiplier(&mut self, multiplier: f32) {
        self.spawn_interval_multiplier = multiplier.max(0.1);
    }

    pub fn effective_spawn_interval(&self, difficulty: f32) -> f32 {
        (self.spawn_interval / difficulty).max(8.0) * self.spawn_interval_multiplier
    }

    fn choose_spawn_x(
        &self,
        player_center_x: f32,
        width: u16,
        difficulty_preset: DifficultyPreset,
        rng: &mut impl Rng,
    ) -> u16 {
        let probability = difficulty_preset.targeted_spawn_probability();
        if probability > 0.0 && rng.random_bool(probability) {
            let radius = difficulty_preset.targeted_spawn_radius();
            let jitter = rng.random_range(-radius..=radius);
            let candidate = centered_spawn_x(player_center_x, width, jitter, self.terminal_width);
            if self.top_spawn_lane_is_clear(candidate, width) {
                return candidate;
            }
        }

        let mut fallback = 0;
        for _ in 0..RANDOM_SPAWN_ATTEMPTS {
            fallback = random_spawn_x(width, self.terminal_width, rng);
            if self.top_spawn_lane_is_clear(fallback, width) {
                return fallback;
            }
        }
        fallback
    }

    fn top_spawn_lane_is_clear(&self, x: u16, width: u16) -> bool {
        let left = x.saturating_sub(SPAWN_HORIZONTAL_GAP) as f32;
        let right = x.saturating_add(width).saturating_add(SPAWN_HORIZONTAL_GAP) as f32;
        self.obstacles.iter().all(|obstacle| {
            !obstacle.active
                || obstacle.y >= obstacle.height as f32
                || right <= obstacle.x
                || left >= obstacle.x + obstacle.width as f32
        })
    }
}

fn centered_spawn_x(
    player_center_x: f32,
    enemy_width: u16,
    jitter: i32,
    terminal_width: u16,
) -> u16 {
    let max_x = terminal_width.saturating_sub(enemy_width) as i32;
    let desired_x = player_center_x.round() as i32 - i32::from(enemy_width / 2) + jitter;
    desired_x.clamp(0, max_x) as u16
}

fn random_spawn_x(width: u16, terminal_width: u16, rng: &mut impl Rng) -> u16 {
    let max_x = terminal_width.saturating_sub(width);
    rng.random_range(0..=max_x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rng;

    fn assert_sprite_is_centered(sprite: &[&str], width: u16) {
        for line in sprite {
            assert_eq!(line.chars().count(), width as usize);
            if let Some(axis) = line.chars().position(|ch| ch == '|') {
                assert_eq!(axis, width as usize / 2);
            }
        }
    }

    fn update_pool(pool: &mut ObstaclePool, dt: f32, rng: &mut impl Rng) {
        pool.update(
            ObstacleUpdateContext {
                terminal_width: 80,
                terminal_height: 30,
                speed_multiplier: 1.0,
                difficulty: 1.0,
                player_center_x: 40.0,
                difficulty_preset: DifficultyPreset::Normal,
                dt,
            },
            rng,
        );
    }

    #[test]
    fn enemy_sprites_use_fixed_width_and_center_axis() {
        assert_sprite_is_centered(BIG_SPRITE, BIG_WIDTH);
        assert_sprite_is_centered(SMALL_SPRITE, SMALL_WIDTH);

        let big = Obstacle::new(ObstacleType::Big, 0.0);
        let small = Obstacle::new(ObstacleType::Small, 0.0);
        assert_eq!(
            (big.width, big.height),
            (BIG_WIDTH, BIG_SPRITE.len() as u16)
        );
        assert_eq!(
            (small.width, small.height),
            (SMALL_WIDTH, SMALL_SPRITE.len() as u16)
        );
    }

    #[test]
    fn enemy_types_have_distinct_base_scores() {
        assert_eq!(ObstacleType::Small.base_score(), 50);
        assert_eq!(ObstacleType::Big.base_score(), 100);
    }

    #[test]
    fn spawn_interval_multiplier_delays_enemy_creation() {
        let mut pool = ObstaclePool::new(80);
        let mut rng = rng();

        pool.set_spawn_interval_multiplier(1.8);
        update_pool(&mut pool, 40.0, &mut rng);
        assert!(pool.obstacles.is_empty());

        update_pool(&mut pool, 32.0, &mut rng);
        assert_eq!(pool.obstacles.len(), 1);
    }

    #[test]
    fn high_difficulty_multiplier_shortens_spawn_interval() {
        let mut pool = ObstaclePool::new(80);
        pool.set_spawn_interval_multiplier(0.65);

        assert_eq!(pool.effective_spawn_interval(1.0), 26.0);
    }

    #[test]
    fn clearing_active_enemies_removes_wave_and_discards_spawn_progress() {
        let mut pool = ObstaclePool::new(80);
        let mut rng = rng();
        pool.obstacles
            .push(Obstacle::new(ObstacleType::Small, 10.0));
        update_pool(&mut pool, 39.0, &mut rng);

        pool.clear_active_enemies();
        update_pool(&mut pool, 1.0, &mut rng);

        assert!(pool.obstacles.iter().all(|obstacle| !obstacle.active));
    }

    #[test]
    fn obstacle_can_store_a_preassigned_reward() {
        let obstacle = Obstacle::with_pickup(ObstacleType::Small, 10.0, Some(PickupKind::Scatter));

        assert_eq!(obstacle.carried_pickup, Some(PickupKind::Scatter));
    }

    #[test]
    fn targeted_spawn_changes_only_x_and_keeps_top_entry_y() {
        assert_eq!(centered_spawn_x(40.0, BIG_WIDTH, 0, 80), 35);
        assert_eq!(centered_spawn_x(2.0, BIG_WIDTH, -8, 80), 0);
        assert_eq!(centered_spawn_x(78.0, BIG_WIDTH, 8, 80), 69);

        let obstacle = Obstacle::new(ObstacleType::Big, 35.0);
        assert_eq!(obstacle.y, -(obstacle.height as f32));
    }

    #[test]
    fn occupied_top_lane_is_rejected_before_random_fallback() {
        let mut pool = ObstaclePool::new(80);
        pool.obstacles.push(Obstacle::new(ObstacleType::Big, 35.0));

        assert!(!pool.top_spawn_lane_is_clear(35, BIG_WIDTH));
        assert!(pool.top_spawn_lane_is_clear(0, SMALL_WIDTH));
    }
}

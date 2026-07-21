use crossterm::style::Color;
use rand::{Rng, RngExt};

pub enum ObstacleType {
    Big,   // slow, wider - heavy bomber
    Small, // fast, narrow - fighter
}

pub struct Obstacle {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub width: u16,
    pub height: u16,
    pub active: bool,
    pub kind: ObstacleType,
}

// Heavy bomber (facing down, coming from top)
const BIG_SPRITE: &[&str] = &[
    "\\_|_/",
    " /*|*\\ ",
    " /**|**\\ ",
    "/***|***\\",
    " \\***|***/ ",
    "  \\*|*/  ",
];

// Small fighter (facing down, coming from top)
const SMALL_SPRITE: &[&str] = &["\\|/", "/*|*\\", " *|* ", "  *  "];

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
    pub fn new(kind: ObstacleType, x: f32) -> Self {
        let (width, height, speed) = match kind {
            ObstacleType::Big => (10, 6, 0.35),
            ObstacleType::Small => (5, 4, 0.7),
        };
        Self {
            x,
            y: -(height as f32),
            speed,
            width,
            height,
            active: true,
            kind,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.y += self.speed * dt;
    }

    pub fn is_off_screen(&self, terminal_height: u16) -> bool {
        self.y > terminal_height as f32
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
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

    pub fn update(
        &mut self,
        terminal_width: u16,
        terminal_height: u16,
        speed_multiplier: f32,
        difficulty: f32,
        dt: f32,
        rng: &mut impl Rng,
    ) {
        self.terminal_width = terminal_width;

        // Spawn timer (frame-time units relative to the 30 FPS baseline)
        self.spawn_timer += dt;
        let interval = (self.spawn_interval / difficulty).max(8.0) * self.spawn_interval_multiplier;
        if self.spawn_timer >= interval {
            self.spawn_timer = 0.0;
            self.spawn(rng);
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

    fn spawn(&mut self, rng: &mut impl Rng) {
        // Find an inactive slot or create new
        let slot = self.obstacles.iter_mut().find(|o| !o.active);
        let kind = if rng.random_bool(0.3) {
            ObstacleType::Big
        } else {
            ObstacleType::Small
        };
        let max_x = self.terminal_width.saturating_sub(10).max(1);
        let x = rng.random_range(0..max_x) as f32;

        match slot {
            Some(obs) => {
                *obs = Obstacle::new(kind, x);
            }
            None => {
                self.obstacles.push(Obstacle::new(kind, x));
            }
        }
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
        for obs in &self.obstacles {
            obs.render(renderer);
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
        self.spawn_interval_multiplier = multiplier.max(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rng;

    #[test]
    fn spawn_interval_multiplier_delays_enemy_creation() {
        let mut pool = ObstaclePool::new(80);
        let mut rng = rng();

        pool.set_spawn_interval_multiplier(1.8);
        pool.update(80, 30, 1.0, 1.0, 40.0, &mut rng);
        assert!(pool.obstacles.is_empty());

        pool.update(80, 30, 1.0, 1.0, 32.0, &mut rng);
        assert_eq!(pool.obstacles.len(), 1);
    }

    #[test]
    fn clearing_active_enemies_removes_wave_and_discards_spawn_progress() {
        let mut pool = ObstaclePool::new(80);
        let mut rng = rng();
        pool.obstacles
            .push(Obstacle::new(ObstacleType::Small, 10.0));
        pool.update(80, 30, 1.0, 1.0, 39.0, &mut rng);

        pool.clear_active_enemies();
        pool.update(80, 30, 1.0, 1.0, 1.0, &mut rng);

        assert!(pool.obstacles.iter().all(|obstacle| !obstacle.active));
    }
}

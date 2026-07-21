use crossterm::style::Color;

pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub width: u16,
    pub height: u16,
    pub active: bool,
    pub horizontal_speed: f32,
    speed: f32,
}

const BULLET_CHAR: char = '|';
const BULLET_COLOR: Color = Color::Cyan;
const BULLET_SPEED: f32 = 3.0;
// Cooldown is expressed in "frames at the 30 FPS design baseline" so the
// effective fire rate stays roughly constant in wall-clock time regardless of
// the actual FPS. `dt` passed to `update`/`fire` is the frame-time factor
// relative to that baseline (1.0 ≈ one 30 FPS frame).
const FIRE_COOLDOWN: f32 = 8.0; // ~4 shots/s at baseline

impl Bullet {
    pub fn new(x: f32, y: f32, horizontal_speed: f32) -> Self {
        Self {
            x,
            y,
            width: 1,
            height: 1,
            active: true,
            horizontal_speed,
            speed: BULLET_SPEED,
        }
    }

    /// Move `dt` (frame-time factor, ~1.0 at 30 FPS) so vertical travel is
    /// frame-rate independent.
    pub fn update(&mut self, terminal_width: u16, dt: f32) {
        self.x += self.horizontal_speed * dt;
        self.y -= self.speed * dt;
        // Off-screen (above top edge or beyond either horizontal edge)
        if self.y < 0.0 || self.x < 0.0 || self.x >= terminal_width as f32 {
            self.active = false;
        }
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
        if !self.active {
            return;
        }
        let x = self.x as u16;
        let y = self.y as u16;
        renderer.put_char(x, y, BULLET_CHAR, BULLET_COLOR);
    }
}

pub struct BulletPool {
    pub bullets: Vec<Bullet>,
    cooldown: f32,
}

impl BulletPool {
    pub fn new() -> Self {
        Self {
            bullets: Vec::new(),
            cooldown: 0.0,
        }
    }

    pub fn update(&mut self, terminal_width: u16, dt: f32) {
        // Decrement cooldown in frame-time units
        if self.cooldown > 0.0 {
            self.cooldown = (self.cooldown - dt).max(0.0);
        }

        // Update active bullets
        for bullet in &mut self.bullets {
            if bullet.active {
                bullet.update(terminal_width, dt);
            }
        }
    }

    pub fn fire(&mut self, x: f32, y: f32, weapon_level: u8) {
        if self.cooldown > 0.0 {
            return;
        }

        let spread: &[f32] = match weapon_level {
            1 => &[0.0],
            2 => &[-0.35, 0.0, 0.35],
            _ => &[-0.70, -0.35, 0.0, 0.35, 0.70],
        };

        for &horizontal_speed in spread {
            // Find an inactive slot or create new. Every projectile in the
            // volley shares the one cooldown applied after this loop.
            if let Some(bullet) = self.bullets.iter_mut().find(|bullet| !bullet.active) {
                *bullet = Bullet::new(x, y, horizontal_speed);
            } else {
                self.bullets.push(Bullet::new(x, y, horizontal_speed));
            }
        }
        self.cooldown = FIRE_COOLDOWN;
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
        for bullet in &self.bullets {
            bullet.render(renderer);
        }
    }

    pub fn clear(&mut self) {
        self.bullets.clear();
        self.cooldown = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_levels_fire_one_three_and_five_projectiles() {
        for (level, expected_count) in [(1, 1), (2, 3), (3, 5)] {
            let mut pool = BulletPool::new();
            pool.fire(20.0, 20.0, level);

            let active: Vec<_> = pool.bullets.iter().filter(|bullet| bullet.active).collect();
            assert_eq!(active.len(), expected_count);
            assert!(
                active
                    .windows(2)
                    .all(|pair| { pair[0].horizontal_speed < pair[1].horizontal_speed })
            );
        }
    }

    #[test]
    fn one_volley_uses_one_shared_cooldown() {
        let mut pool = BulletPool::new();
        pool.fire(20.0, 50.0, 3);
        pool.fire(20.0, 50.0, 3);

        assert_eq!(
            pool.bullets.iter().filter(|bullet| bullet.active).count(),
            5
        );

        pool.update(80, FIRE_COOLDOWN);
        pool.fire(20.0, 50.0, 3);
        assert_eq!(
            pool.bullets.iter().filter(|bullet| bullet.active).count(),
            10
        );
    }

    #[test]
    fn angled_bullets_deactivate_outside_horizontal_bounds() {
        let mut bullet = Bullet::new(0.1, 20.0, -0.35);

        bullet.update(80, 1.0);

        assert!(!bullet.active);
    }

    #[test]
    fn pool_reuses_inactive_bullet_slots() {
        let mut pool = BulletPool::new();
        pool.fire(10.0, 50.0, 1);
        pool.bullets[0].active = false;
        pool.update(80, FIRE_COOLDOWN);

        pool.fire(30.0, 50.0, 1);

        assert_eq!(pool.bullets.len(), 1);
        assert_eq!(pool.bullets[0].x, 30.0);
        assert!(pool.bullets[0].active);
    }
}

use crossterm::style::Color;
use rand::{Rng, RngExt};

const PICKUP_WIDTH: u16 = 3;
const PICKUP_HEIGHT: u16 = 1;
const PICKUP_SPEED: f32 = 0.45;

pub const PICKUP_DROP_CHANCE: f64 = 0.20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Scatter,
    Repair,
    Emp,
}

impl PickupKind {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Scatter => "[S]",
            Self::Repair => "[H]",
            Self::Emp => "[E]",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Scatter => Color::Magenta,
            Self::Repair => Color::Red,
            Self::Emp => Color::Cyan,
        }
    }
}

/// Decide whether a newly spawned enemy carries a reward. Assignment happens
/// at enemy creation in every mode, so debug rendering can observe the result
/// without the debug flag changing random draws or drop rules.
pub fn random_drop(rng: &mut impl Rng) -> Option<PickupKind> {
    rng.random_bool(PICKUP_DROP_CHANCE)
        .then(|| random_kind(rng))
}

/// Choose a reward after the separate 20% drop roll succeeds:
/// Scatter 55%, Repair 15%, EMP 30%.
pub fn random_kind(rng: &mut impl Rng) -> PickupKind {
    kind_for_roll(rng.random_range(0..100))
}

fn kind_for_roll(roll: u8) -> PickupKind {
    match roll {
        0..=54 => PickupKind::Scatter,
        55..=69 => PickupKind::Repair,
        _ => PickupKind::Emp,
    }
}

pub struct Pickup {
    pub kind: PickupKind,
    pub x: f32,
    pub y: f32,
    pub width: u16,
    pub height: u16,
    pub active: bool,
}

impl Pickup {
    fn new(kind: PickupKind, x: f32, y: f32) -> Self {
        Self {
            kind,
            x,
            y,
            width: PICKUP_WIDTH,
            height: PICKUP_HEIGHT,
            active: true,
        }
    }

    fn update(&mut self, terminal_height: u16, dt: f32) {
        self.y += PICKUP_SPEED * dt;
        if self.y >= terminal_height as f32 {
            self.active = false;
        }
    }

    pub fn hitbox(&self) -> (u16, u16, u16, u16) {
        (self.x as u16, self.y as u16, self.width, self.height)
    }

    fn render(&self, renderer: &mut crate::renderer::Renderer) {
        if self.active {
            renderer.put_str(
                self.x as u16,
                self.y as u16,
                self.kind.symbol(),
                self.kind.color(),
            );
        }
    }
}

pub struct PickupPool {
    pub pickups: Vec<Pickup>,
}

impl PickupPool {
    pub fn new() -> Self {
        Self {
            pickups: Vec::new(),
        }
    }

    pub fn spawn(&mut self, kind: PickupKind, x: f32, y: f32) {
        if let Some(pickup) = self.pickups.iter_mut().find(|pickup| !pickup.active) {
            *pickup = Pickup::new(kind, x, y);
        } else {
            self.pickups.push(Pickup::new(kind, x, y));
        }
    }

    pub fn update(&mut self, terminal_height: u16, dt: f32) {
        for pickup in &mut self.pickups {
            if pickup.active {
                pickup.update(terminal_height, dt);
            }
        }
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
        for pickup in &self.pickups {
            pickup.render(renderer);
        }
    }

    pub fn clear(&mut self) {
        self.pickups.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_reuses_inactive_slots_and_clear_removes_all_entries() {
        let mut pool = PickupPool::new();
        pool.spawn(PickupKind::Scatter, 4.0, 5.0);
        pool.pickups[0].active = false;

        pool.spawn(PickupKind::Scatter, 8.0, 9.0);

        assert_eq!(pool.pickups.len(), 1);
        assert_eq!(pool.pickups[0].x, 8.0);
        assert!(pool.pickups[0].active);

        pool.clear();
        assert!(pool.pickups.is_empty());
    }

    #[test]
    fn pickup_moves_with_dt_and_deactivates_below_screen() {
        let mut pool = PickupPool::new();
        pool.spawn(PickupKind::Scatter, 4.0, 9.0);

        pool.update(10, 1.0);

        assert_eq!(pool.pickups[0].y, 9.45);
        assert!(pool.pickups[0].active);

        pool.update(10, 2.0);
        assert!(!pool.pickups[0].active);
    }

    #[test]
    fn reward_rolls_follow_configured_weight_boundaries() {
        assert!(matches!(kind_for_roll(0), PickupKind::Scatter));
        assert!(matches!(kind_for_roll(54), PickupKind::Scatter));
        assert!(matches!(kind_for_roll(55), PickupKind::Repair));
        assert!(matches!(kind_for_roll(69), PickupKind::Repair));
        assert!(matches!(kind_for_roll(70), PickupKind::Emp));
        assert!(matches!(kind_for_roll(99), PickupKind::Emp));
    }
}

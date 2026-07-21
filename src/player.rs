use crossterm::style::Color;

pub struct Player {
    pub x: f32,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub alive: bool,
    pub weapon_level: u8,
    move_speed: f32,
}

pub const MAX_WEAPON_LEVEL: u8 = 3;

// Top-down fighter jet: nose → wings → tail
const SPRITE: &[&str] = &[
    "    ^    ",  // nose
    "   /|\\   ", // cockpit
    "  /*|*\\  ", // wing roots
    " /**|**\\ ", // wings
    "/***|***\\", // wing tips
    "  /*|*\\  ", // rear body
    "  /_|_\\  ", // tail
];

const SPRITE_WIDTH: u16 = 9;
const SPRITE_HEIGHT: u16 = 7;

// Color map: which color to use for each character
fn char_color(ch: char) -> Color {
    match ch {
        '*' => Color::Blue,            // wings
        '/' | '\\' => Color::DarkGrey, // wing edges
        '^' => Color::Yellow,          // nose tip
        '|' => Color::White,           // fuselage
        '_' => Color::DarkGrey,        // tail
        _ => Color::Reset,
    }
}

// Four direction enum. `Left`/`Right` are horizontal, `Up`/`Down` are vertical.
// `move_in_dir` takes one horizontal and one vertical direction so both axes
// can be held at once, and the held-state model in `main.rs` can treat all four
// directions uniformly.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Player {
    pub fn new(terminal_width: u16, terminal_height: u16) -> Self {
        Self {
            x: (terminal_width as f32 - SPRITE_WIDTH as f32) / 2.0,
            y: terminal_height.saturating_sub(SPRITE_HEIGHT + 2),
            width: SPRITE_WIDTH,
            height: SPRITE_HEIGHT,
            alive: true,
            weapon_level: 1,
            move_speed: 3.0,
        }
    }

    /// Move based on a horizontal and a vertical direction, each decided by the
    /// caller (`main.rs`) so that the most-recently-pressed key wins per axis
    /// and releasing falls back to the other still-held key. `None` on an axis
    /// means no movement on that axis. Movement is frame-rate independent:
    /// `dt` (seconds since last frame) scales speed, so holding a key travels
    /// the same distance per real second regardless of FPS.
    pub fn move_in_dir(
        &mut self,
        h_dir: Option<Dir>,
        v_dir: Option<Dir>,
        terminal_width: u16,
        terminal_height: u16,
        dt: f32,
    ) {
        let max_x = terminal_width.saturating_sub(self.width) as f32;
        let max_y = terminal_height.saturating_sub(self.height) as f32;
        let step = self.move_speed * dt;
        match h_dir {
            Some(Dir::Left) => self.x = (self.x - step).max(0.0),
            Some(Dir::Right) => self.x = (self.x + step).min(max_x),
            _ => {}
        }
        match v_dir {
            Some(Dir::Up) => self.y = (self.y as f32 - step).max(0.0) as u16,
            Some(Dir::Down) => self.y = ((self.y as f32 + step).min(max_y)) as u16,
            _ => {}
        }
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
        if !self.alive {
            return;
        }
        let x = self.x as u16;
        for (row, line) in SPRITE.iter().enumerate() {
            let y = self.y + row as u16;
            for (col, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    let color = char_color(ch);
                    renderer.put_char(x + col as u16, y, ch, color);
                }
            }
        }
    }

    /// Returns (x, y, width, height) for collision detection
    pub fn hitbox(&self) -> (u16, u16, u16, u16) {
        (self.x as u16, self.y, self.width, self.height)
    }

    pub fn upgrade_weapon(&mut self) -> bool {
        if self.weapon_level < MAX_WEAPON_LEVEL {
            self.weapon_level += 1;
            true
        } else {
            false
        }
    }

    pub fn update_terminal_size(&mut self, _width: u16, height: u16) {
        self.y = height.saturating_sub(SPRITE_HEIGHT + 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_upgrades_to_level_three_and_no_further() {
        let mut player = Player::new(80, 24);

        assert!(player.upgrade_weapon());
        assert!(player.upgrade_weapon());
        assert!(!player.upgrade_weapon());
        assert_eq!(player.weapon_level, MAX_WEAPON_LEVEL);
    }
}

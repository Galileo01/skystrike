use crossterm::style::Color;
use rand::{Rng, RngExt};

struct Star {
    x: u16,
    y: f32,
    speed: f32,
    ch: char,
    color: Color,
}

pub struct Background {
    stars: Vec<Star>,
    width: u16,
    height: u16,
}

impl Background {
    pub fn new(width: u16, height: u16, rng: &mut impl Rng) -> Self {
        let mut stars = Vec::new();
        // Far layer - slow, dim
        for _ in 0..(width / 3) {
            stars.push(Star {
                x: rng.random_range(0..width),
                y: rng.random_range(0.0..height as f32),
                speed: 0.15,
                ch: '.',
                color: Color::DarkGrey,
            });
        }
        // Near layer - fast, bright
        for _ in 0..(width / 6) {
            stars.push(Star {
                x: rng.random_range(0..width),
                y: rng.random_range(0.0..height as f32),
                speed: 0.4,
                ch: '*',
                color: Color::White,
            });
        }
        Self { stars, width, height }
    }

    pub fn update(&mut self, width: u16, height: u16, dt: f32, rng: &mut impl Rng) {
        self.width = width;
        self.height = height;
        for star in &mut self.stars {
            star.y += star.speed * dt;
            if star.y >= height as f32 {
                star.y = 0.0;
                star.x = rng.random_range(0..width);
            }
        }
    }

    pub fn render(&self, renderer: &mut crate::renderer::Renderer) {
        for star in &self.stars {
            renderer.put_char(star.x, star.y as u16, star.ch, star.color);
        }
    }

    pub fn resize(&mut self, width: u16, height: u16, rng: &mut impl Rng) {
        *self = Self::new(width, height, rng);
    }
}

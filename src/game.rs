use crossterm::style::Color;
use rand::Rng;

use crate::background::Background;
use crate::bullet::BulletPool;
use crate::difficulty::{DifficultyPreset, HighScores, PlayerProfile};
use crate::obstacle::{ObstaclePool, ObstacleUpdateContext};
use crate::pickup::{PickupKind, PickupPool};
use crate::player::Player;
use crate::renderer::Renderer;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Menu,
    Playing,
    Paused,
    GameOver,
}

const INVINCIBLE_TICKS: u32 = 120; // ~2s of blink after a hit (at 60 FPS)
const COMBO_WINDOW: u32 = 180; // combo resets if no kill within ~3s (at 60 FPS)
const MAX_LIVES: u8 = 3;
const SCATTER_DURATION_TICKS: u32 = 600; // ~10s at 60 FPS
const EMP_DURATION_TICKS: u32 = 600; // ~10s at 60 FPS
const EMP_SPAWN_INTERVAL_MULTIPLIER: f32 = 1.8;
const PICKUP_NOTICE_TICKS: u32 = 120; // ~2s at 60 FPS
const SCORE_POPUP_TICKS: u32 = 45; // ~0.75s at 60 FPS

pub struct PickupNotice {
    pub text: String,
    pub color: Color,
    pub ticks: u32,
}

struct ScorePopup {
    score: u32,
    x: u16,
    y: u16,
    ticks: u32,
}

pub struct Game {
    pub state: GameState,
    pub player: Player,
    pub bullets: BulletPool,
    pub obstacles: ObstaclePool,
    pub pickups: PickupPool,
    pub background: Background,
    pub score: u32,
    pub difficulty: f32,
    pub difficulty_preset: DifficultyPreset,
    pub width: u16,
    pub height: u16,
    pub lives: u8,
    pub invincible_ticks: u32,
    pub combo: u32,
    pub combo_timer: u32,
    pub scatter_ticks: u32,
    pub emp_ticks: u32,
    pub pickup_notice: Option<PickupNotice>,
    pub debug_enabled: bool,
    high_scores: HighScores,
    score_popups: Vec<ScorePopup>,
}

impl Game {
    pub fn new(width: u16, height: u16, rng: &mut impl Rng) -> Self {
        Self {
            state: GameState::Menu,
            player: Player::new(width, height),
            bullets: BulletPool::new(),
            obstacles: ObstaclePool::new(width),
            pickups: PickupPool::new(),
            background: Background::new(width, height, rng),
            score: 0,
            difficulty: 1.0,
            difficulty_preset: DifficultyPreset::Normal,
            width,
            height,
            lives: MAX_LIVES,
            invincible_ticks: 0,
            combo: 0,
            combo_timer: 0,
            scatter_ticks: 0,
            emp_ticks: 0,
            pickup_notice: None,
            debug_enabled: false,
            high_scores: HighScores::default(),
            score_popups: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        self.state = GameState::Playing;
        self.score = 0;
        self.difficulty = 1.0;
        self.lives = MAX_LIVES;
        self.invincible_ticks = 0;
        self.combo = 0;
        self.combo_timer = 0;
        self.scatter_ticks = 0;
        self.emp_ticks = 0;
        self.pickup_notice = None;
        self.score_popups.clear();
        self.player = Player::new(self.width, self.height);
        self.bullets.clear();
        self.obstacles.clear();
        self.pickups.clear();
    }

    pub fn player_fire(&mut self) {
        if self.state != GameState::Playing {
            return;
        }
        // Fire from nose tip — player sprite is 9 wide, nose at x+4, above y-1
        let bx = self.player.x + 4.0;
        let by = self.player.y as f32 - 1.0;
        self.bullets.fire(bx, by, self.player.weapon_level);
    }

    pub fn pause(&mut self) {
        if self.state == GameState::Playing {
            self.state = GameState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == GameState::Paused {
            self.state = GameState::Playing;
        }
    }

    pub fn return_to_menu(&mut self) {
        if matches!(self.state, GameState::Playing | GameState::Paused) {
            self.finish_run();
            self.state = GameState::Menu;
        }
    }

    pub fn finish_run(&mut self) {
        self.high_scores
            .update_if_higher(self.difficulty_preset, self.score);
    }

    pub fn apply_profile(&mut self, profile: PlayerProfile) {
        self.difficulty_preset = profile.selected_difficulty;
        self.high_scores = profile.high_scores;
    }

    pub fn profile(&self) -> PlayerProfile {
        PlayerProfile {
            selected_difficulty: self.difficulty_preset,
            high_scores: self.high_scores,
        }
    }

    pub fn current_high_score(&self) -> u32 {
        self.high_scores.get(self.difficulty_preset)
    }

    pub fn select_difficulty(&mut self, preset: DifficultyPreset) {
        if self.state == GameState::Menu {
            self.difficulty_preset = preset;
        }
    }

    pub fn set_debug_enabled(&mut self, enabled: bool) {
        self.debug_enabled = enabled;
    }

    pub fn update(&mut self, dt: f32, rng: &mut impl Rng) {
        // Pause freezes the entire simulation, including the starfield.
        if self.state == GameState::Paused {
            return;
        }

        // Starfield scrolls in every state (menu / playing / game over) so the
        // screen always looks alive instead of frozen on the title screen.
        self.background.update(self.width, self.height, dt, rng);

        if self.state != GameState::Playing {
            return;
        }

        if let Some(notice) = &mut self.pickup_notice {
            notice.ticks = notice.ticks.saturating_sub(1);
            if notice.ticks == 0 {
                self.pickup_notice = None;
            }
        }
        if self.scatter_ticks > 0 {
            self.scatter_ticks -= 1;
            if self.scatter_ticks == 0 {
                self.player.weapon_level = 1;
                self.pickup_notice = Some(PickupNotice {
                    text: "SCATTER EXPIRED - WPN LV1".to_owned(),
                    color: Color::DarkMagenta,
                    ticks: PICKUP_NOTICE_TICKS,
                });
            }
        }
        if self.emp_ticks > 0 {
            self.emp_ticks -= 1;
        }
        for popup in &mut self.score_popups {
            popup.ticks = popup.ticks.saturating_sub(1);
        }
        self.score_popups.retain(|popup| popup.ticks > 0);

        self.bullets.update(self.width, dt);
        self.pickups.update(self.height, dt);

        // Difficulty ramps up over time
        self.difficulty = 1.0 + (self.score as f32 / 800.0).min(2.5);
        let speed_multiplier = self.enemy_speed_multiplier();

        let spawn_interval_multiplier = self.spawn_interval_multiplier();
        self.obstacles
            .set_spawn_interval_multiplier(spawn_interval_multiplier);
        self.obstacles.update(
            ObstacleUpdateContext {
                terminal_width: self.width,
                terminal_height: self.height,
                speed_multiplier,
                difficulty: self.difficulty,
                player_center_x: self.player.x + self.player.width as f32 / 2.0,
                difficulty_preset: self.difficulty_preset,
                dt,
            },
            rng,
        );

        // Collision detection — player vs obstacles
        let (px, py, pw, ph) = self.player.hitbox();
        for obs in &mut self.obstacles.obstacles {
            if !obs.active {
                continue;
            }
            let (ox, oy, ow, oh) = obs.hitbox();
            if rects_overlap(px, py, pw, ph, ox, oy, ow, oh) {
                if self.invincible_ticks == 0 {
                    // Keep the player's chosen position, remove the colliding
                    // enemy, and grant a short invincibility window. Teleporting
                    // into an uncleared center lane caused unfair follow-up hits.
                    self.lives = self.lives.saturating_sub(1);
                    self.invincible_ticks = INVINCIBLE_TICKS;
                    obs.active = false;
                    if self.lives == 0 {
                        self.state = GameState::GameOver;
                        self.high_scores
                            .update_if_higher(self.difficulty_preset, self.score);
                        return;
                    }
                    self.pickup_notice = Some(PickupNotice {
                        text: format!("HIT - LIFE {}", self.lives),
                        color: Color::Red,
                        ticks: PICKUP_NOTICE_TICKS,
                    });
                }
                // While invincible we ignore further collisions this frame.
                break;
            }
        }

        // Collision detection — player vs pickups. Weapon progress survives a
        // lost life, but start() creates a fresh level-one Player next game.
        let (px, py, pw, ph) = self.player.hitbox();
        for pickup in &mut self.pickups.pickups {
            if !pickup.active {
                continue;
            }
            let (kx, ky, kw, kh) = pickup.hitbox();
            if rects_overlap(px, py, pw, ph, kx, ky, kw, kh) {
                pickup.active = false;
                match pickup.kind {
                    PickupKind::Scatter => {
                        let text = if self.player.upgrade_weapon() {
                            self.scatter_ticks = SCATTER_DURATION_TICKS;
                            format!("SCATTER - WPN LV{}", self.player.weapon_level)
                        } else {
                            self.score += 500;
                            "SCATTER - MAX, +500 SCORE".to_owned()
                        };
                        self.pickup_notice = Some(PickupNotice {
                            text,
                            color: pickup.kind.color(),
                            ticks: PICKUP_NOTICE_TICKS,
                        });
                    }
                    PickupKind::Repair => {
                        let text = if self.lives < MAX_LIVES {
                            self.lives += 1;
                            "REPAIR - LIFE +1".to_owned()
                        } else {
                            self.score += 300;
                            "REPAIR - FULL, +300 SCORE".to_owned()
                        };
                        self.pickup_notice = Some(PickupNotice {
                            text,
                            color: pickup.kind.color(),
                            ticks: PICKUP_NOTICE_TICKS,
                        });
                    }
                    PickupKind::Emp => {
                        self.emp_ticks = EMP_DURATION_TICKS;
                        // EMP has an immediate, unmistakable pulse, then keeps
                        // suppressing future spawns. Cleared enemies grant no
                        // score, combo, or additional drops.
                        self.obstacles.clear_active_enemies();
                        self.pickup_notice = Some(PickupNotice {
                            text: "EMP BURST - SCREEN CLEARED".to_owned(),
                            color: pickup.kind.color(),
                            ticks: PICKUP_NOTICE_TICKS,
                        });
                    }
                }
            }
        }

        // Collision detection — bullets vs obstacles
        for bullet in &mut self.bullets.bullets {
            if !bullet.active {
                continue;
            }
            let (bx, by, bw, bh) = (
                bullet.x as u16,
                bullet.y as u16,
                bullet.width,
                bullet.height,
            );
            for obs in &mut self.obstacles.obstacles {
                if !obs.active {
                    continue;
                }
                let (ox, oy, ow, oh) = obs.hitbox();
                if rects_overlap(bx, by, bw, bh, ox, oy, ow, oh) {
                    let drop_x = (obs.x + obs.width as f32 / 2.0 - 1.0)
                        .clamp(0.0, self.width.saturating_sub(3) as f32);
                    let drop_y = obs.y.max(0.0);
                    bullet.active = false;
                    obs.active = false;
                    if let Some(kind) = obs.carried_pickup.take() {
                        self.pickups.spawn(kind, drop_x, drop_y);
                    }
                    if self.combo_timer > 0 {
                        self.combo += 1;
                    } else {
                        self.combo = 1;
                    }
                    self.combo_timer = COMBO_WINDOW;
                    let earned_score = obs.kind.base_score() * self.combo;
                    self.score += earned_score;
                    self.score_popups.push(ScorePopup {
                        score: earned_score,
                        x: obs.x as u16 + obs.width / 2,
                        y: (obs.y.max(4.0) as u16).min(self.height.saturating_sub(1)),
                        ticks: SCORE_POPUP_TICKS,
                    });
                    break;
                }
            }
        }

        if self.invincible_ticks > 0 {
            self.invincible_ticks -= 1;
        }
        if self.combo_timer > 0 {
            self.combo_timer -= 1;
            if self.combo_timer == 0 {
                self.combo = 0;
            }
        }

        self.score += 1;
    }

    pub fn render(&self, renderer: &mut Renderer) {
        match self.state {
            GameState::Menu => {
                self.background.render(renderer);
                self.render_title(renderer);
            }
            GameState::Playing | GameState::Paused => {
                self.background.render(renderer);
                self.bullets.render(renderer);
                self.obstacles.render(renderer, self.debug_enabled);
                self.pickups.render(renderer);
                // Blink while invincible: skip rendering on odd invincibility
                // ticks so the player flickers instead of staying solid.
                let blink = self.invincible_ticks > 0 && self.invincible_ticks % 2 == 1;
                if !blink {
                    self.player.render(renderer);
                }
                self.render_hud(renderer);
                self.render_debug_hud(renderer);
                self.render_pickup_notice(renderer);
                self.render_score_popups(renderer);
                if self.state == GameState::Paused {
                    self.render_paused(renderer);
                }
            }
            GameState::GameOver => {
                self.background.render(renderer);
                self.obstacles.render(renderer, self.debug_enabled);
                self.render_game_over(renderer);
            }
        }
    }

    fn render_title(&self, renderer: &mut Renderer) {
        let cx = self.width / 2;
        let cy = self.height / 2;

        let title = [
            " ____  _  __ __   __  ____ _____ ____  ___ _  _______",
            "/ ___|| |/ / \\ \\ / / / ___|_   _|  _ \\|_ _| |/ / ____|",
            "\\___ \\| ' /   \\ V /  \\___ \\ | | | |_) || || ' /|  _|",
            " ___) | . \\    | |    ___) || | |  _ < | || . \\| |___",
            "|____/|_|\\_\\   |_|   |____/ |_| |_| \\_\\___|_|\\_\\_____|",
        ];

        let start_y = cy.saturating_sub(title.len() as u16 + 2);
        for (i, line) in title.iter().enumerate() {
            let x = cx.saturating_sub(line.len() as u16 / 2);
            renderer.put_str(x, start_y + i as u16, line, Color::Yellow);
        }

        let prompt = "Press SPACE to start";
        let px = cx.saturating_sub(prompt.len() as u16 / 2);
        renderer.put_str(px, start_y + title.len() as u16 + 2, prompt, Color::White);

        let difficulty = format!("DIFFICULTY: < {} >", self.difficulty_preset.label());
        let difficulty_x = cx.saturating_sub(difficulty.len() as u16 / 2);
        renderer.put_str(
            difficulty_x,
            start_y + title.len() as u16 + 4,
            &difficulty,
            self.difficulty_color(),
        );

        let difficulty_hint = "1/2/3/4 or A/D/Arrows to select";
        let hint_x = cx.saturating_sub(difficulty_hint.len() as u16 / 2);
        renderer.put_str(
            hint_x,
            start_y + title.len() as u16 + 6,
            difficulty_hint,
            Color::DarkGrey,
        );

        let controls = "WASD/Arrows: Move  |  J: Fire  |  K: Auto-fire  |  P: Pause  |  ESC: Menu  |  Space: Start  |  Q: Quit";
        let ctrl_x = cx.saturating_sub(controls.len() as u16 / 2);
        renderer.put_str(
            ctrl_x,
            start_y + title.len() as u16 + 8,
            controls,
            Color::DarkGrey,
        );

        let hs = format!(
            "{} High Score: {}",
            self.difficulty_preset.label(),
            self.current_high_score()
        );
        let hs_x = cx.saturating_sub(hs.len() as u16 / 2);
        renderer.put_str(hs_x, start_y + title.len() as u16 + 10, &hs, Color::Cyan);
    }

    fn render_hud(&self, renderer: &mut Renderer) {
        let score_text = format!("SCORE: {}", self.score);
        renderer.put_str(2, 0, &score_text, Color::White);

        let diff_text = format!(
            "{} LV {:.0}",
            self.difficulty_preset.label(),
            self.difficulty
        );
        renderer.put_str(
            self.width.saturating_sub(diff_text.len() as u16 + 2),
            0,
            &diff_text,
            self.difficulty_color(),
        );

        // Lives as heart blocks on the second line.
        let lives_text = "♥".repeat(self.lives as usize);
        renderer.put_str(2, 1, &lives_text, Color::Red);
        let weapon_text = if self.scatter_ticks > 0 {
            let seconds = self.scatter_ticks.div_ceil(60);
            format!("WPN LV{} {seconds}s", self.player.weapon_level)
        } else {
            format!("WPN LV{}", self.player.weapon_level)
        };
        let weapon_x = 2 + lives_text.chars().count() as u16 + 2;
        let weapon_color = if self.scatter_ticks > 0 && self.scatter_ticks <= 180 {
            Color::Red
        } else {
            Color::Magenta
        };
        renderer.put_str(weapon_x, 1, &weapon_text, weapon_color);

        if self.emp_ticks > 0 {
            let seconds = self.emp_ticks.div_ceil(60);
            let emp_text = format!("EMP {seconds}s");
            let emp_x = weapon_x + weapon_text.len() as u16 + 2;
            renderer.put_str(emp_x, 1, &emp_text, Color::Cyan);
        }
    }

    fn render_pickup_notice(&self, renderer: &mut Renderer) {
        let Some(notice) = &self.pickup_notice else {
            return;
        };
        let x = self.width.saturating_sub(notice.text.len() as u16) / 2;
        renderer.put_str(x, 3, &notice.text, notice.color);
    }

    fn render_debug_hud(&self, renderer: &mut Renderer) {
        if !self.debug_enabled {
            return;
        }

        let text = self.debug_hud_text();
        renderer.put_str(2, 2, &text, Color::DarkGrey);
    }

    fn render_score_popups(&self, renderer: &mut Renderer) {
        for popup in &self.score_popups {
            let text = format!("+{}", popup.score);
            let max_x = self.width.saturating_sub(text.len() as u16);
            let x = popup.x.saturating_sub(text.len() as u16 / 2).min(max_x);
            renderer.put_str(x, popup.y, &text, Color::Yellow);
        }
    }

    fn debug_hud_text(&self) -> String {
        let obstacles = self
            .obstacles
            .obstacles
            .iter()
            .filter(|obstacle| obstacle.active)
            .count();
        let bullets = self
            .bullets
            .bullets
            .iter()
            .filter(|bullet| bullet.active)
            .count();
        let pickups = self
            .pickups
            .pickups
            .iter()
            .filter(|pickup| pickup.active)
            .count();
        let spawn_interval = self.obstacles.effective_spawn_interval(self.difficulty);
        format!(
            "DEBUG | MODE {} | DIFF {:.2} | SPEED {:.2} | OBS {obstacles} | BUL {bullets} | PWR {pickups} | SPAWN {spawn_interval:.1}",
            self.difficulty_preset.label(),
            self.difficulty,
            self.enemy_speed_multiplier(),
        )
    }

    fn spawn_interval_multiplier(&self) -> f32 {
        let emp_multiplier = if self.emp_ticks > 0 {
            EMP_SPAWN_INTERVAL_MULTIPLIER
        } else {
            1.0
        };
        self.difficulty_preset.spawn_interval_multiplier() * emp_multiplier
    }

    fn enemy_speed_multiplier(&self) -> f32 {
        let score_multiplier = 0.6 + (self.score as f32 / 2000.0).min(1.0);
        score_multiplier * self.difficulty_preset.enemy_speed_multiplier()
    }

    fn difficulty_color(&self) -> Color {
        match self.difficulty_preset {
            DifficultyPreset::Easy => Color::Green,
            DifficultyPreset::Normal => Color::Yellow,
            DifficultyPreset::Hard => Color::Red,
            DifficultyPreset::Extreme => Color::Magenta,
        }
    }

    fn render_game_over(&self, renderer: &mut Renderer) {
        let cx = self.width / 2;
        let cy = self.height / 2;

        let lines = [
            "  ____    _    __  __ _____    _____     _______ ____  ",
            " / ___|  / \\  |  \\/  | ____|  / _ \\ \\   / / ____|  _ \\ ",
            "| |  _  / _ \\ | |\\/| |  _|   | | | \\ \\ / /|  _| | |_) |",
            "| |_| |/ ___ \\| |  | | |___  | |_| |\\ V / | |___|  _ < ",
            " \\____/_/   \\_\\_|  |_|_____|  \\___/  \\_/  |_____|_| \\_\\",
        ];

        let start_y = cy.saturating_sub(lines.len() as u16 + 3);
        for (i, line) in lines.iter().enumerate() {
            let x = cx.saturating_sub(line.len() as u16 / 2);
            renderer.put_str(x, start_y + i as u16, line, Color::Red);
        }

        let score_line = format!(
            "{} Score: {}  |  High Score: {}",
            self.difficulty_preset.label(),
            self.score,
            self.current_high_score()
        );
        let sx = cx.saturating_sub(score_line.len() as u16 / 2);
        renderer.put_str(
            sx,
            start_y + lines.len() as u16 + 2,
            &score_line,
            Color::Yellow,
        );

        let restart = "Press SPACE to restart  |  Q to quit";
        let rx = cx.saturating_sub(restart.len() as u16 / 2);
        renderer.put_str(rx, start_y + lines.len() as u16 + 4, restart, Color::White);
    }

    fn render_paused(&self, renderer: &mut Renderer) {
        let title = "PAUSED";
        let controls = "P: Resume  |  ESC: Menu";
        let x = self.width.saturating_sub(title.len() as u16) / 2;
        let controls_x = self.width.saturating_sub(controls.len() as u16) / 2;
        let y = self.height / 2;

        renderer.put_str(x, y, title, Color::Yellow);
        renderer.put_str(controls_x, y.saturating_add(2), controls, Color::White);
    }

    pub fn resize(&mut self, width: u16, height: u16, rng: &mut impl Rng) {
        self.width = width;
        self.height = height;
        self.player.update_terminal_size(width, height);
        self.background.resize(width, height, rng);
    }
}

#[allow(clippy::too_many_arguments)]
fn rects_overlap(x1: u16, y1: u16, w1: u16, h1: u16, x2: u16, y2: u16, w2: u16, h2: u16) -> bool {
    x1 < x2 + w2 && x1 + w1 > x2 && y1 < y2 + h2 && y1 + h1 > y2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bullet::Bullet;
    use crate::obstacle::{Obstacle, ObstacleType};
    use rand::rng;

    fn playing_game() -> Game {
        let mut rng = rng();
        let mut game = Game::new(80, 30, &mut rng);
        game.start();
        game
    }

    fn spawn_pickup_on_player(game: &mut Game, kind: PickupKind) {
        game.pickups
            .spawn(kind, game.player.x, game.player.y as f32);
    }

    fn shoot_enemy(game: &mut Game, kind: ObstacleType) {
        let mut obstacle = Obstacle::new(kind, 10.0);
        obstacle.y = 10.0;
        game.obstacles.obstacles.push(obstacle);
        game.bullets.bullets.push(Bullet::new(10.0, 10.0, 0.0));
    }

    #[test]
    fn collecting_scatter_upgrades_weapon() {
        let mut game = playing_game();
        let mut rng = rng();
        spawn_pickup_on_player(&mut game, PickupKind::Scatter);

        game.update(0.0, &mut rng);

        assert_eq!(game.player.weapon_level, 2);
        assert_eq!(game.scatter_ticks, SCATTER_DURATION_TICKS);
        assert!(!game.pickups.pickups[0].active);
    }

    #[test]
    fn collecting_scatter_refreshes_on_upgrade_but_not_at_max_level() {
        let mut game = playing_game();
        let mut rng = rng();

        game.player.weapon_level = 2;
        game.scatter_ticks = 20;
        spawn_pickup_on_player(&mut game, PickupKind::Scatter);
        game.update(0.0, &mut rng);

        assert_eq!(game.player.weapon_level, 3);
        assert_eq!(game.scatter_ticks, SCATTER_DURATION_TICKS);

        game.scatter_ticks = 20;
        game.player.weapon_level = 3;
        spawn_pickup_on_player(&mut game, PickupKind::Scatter);
        game.update(0.0, &mut rng);

        assert_eq!(game.player.weapon_level, 3);
        assert_eq!(game.scatter_ticks, 19);
        assert_eq!(game.score, 502); // two survival points + 500 pickup bonus
    }

    #[test]
    fn scatter_expires_to_level_one_and_pause_freezes_it() {
        let mut game = playing_game();
        let mut rng = rng();
        game.player.weapon_level = 2;
        game.scatter_ticks = 2;

        game.pause();
        game.update(0.0, &mut rng);
        assert_eq!(game.scatter_ticks, 2);
        assert_eq!(game.player.weapon_level, 2);

        game.resume();
        game.update(0.0, &mut rng);
        assert_eq!(game.scatter_ticks, 1);
        assert_eq!(game.player.weapon_level, 2);

        game.update(0.0, &mut rng);
        assert_eq!(game.scatter_ticks, 0);
        assert_eq!(game.player.weapon_level, 1);
        assert_eq!(
            game.pickup_notice
                .as_ref()
                .map(|notice| notice.text.as_str()),
            Some("SCATTER EXPIRED - WPN LV1")
        );
    }

    #[test]
    fn start_resets_weapon_level_and_clears_pickups() {
        let mut game = playing_game();
        game.player.weapon_level = 3;
        game.scatter_ticks = 100;
        game.pickups.spawn(PickupKind::Scatter, 10.0, 10.0);
        game.score_popups.push(ScorePopup {
            score: 50,
            x: 10,
            y: 10,
            ticks: SCORE_POPUP_TICKS,
        });

        game.start();

        assert_eq!(game.player.weapon_level, 1);
        assert_eq!(game.scatter_ticks, 0);
        assert!(game.pickups.pickups.is_empty());
        assert!(game.score_popups.is_empty());
    }

    #[test]
    fn pause_freezes_pickups() {
        let mut game = playing_game();
        let mut rng = rng();
        game.pickups.spawn(PickupKind::Scatter, 10.0, 10.0);
        game.pause();

        game.update(3.0, &mut rng);

        assert_eq!(game.pickups.pickups[0].y, 10.0);
    }

    #[test]
    fn losing_a_life_preserves_weapon_level() {
        let mut game = playing_game();
        let mut rng = rng();
        game.player.weapon_level = 3;
        game.scatter_ticks = 100;
        let original_position = (game.player.x, game.player.y);
        let mut obstacle = Obstacle::new(ObstacleType::Small, game.player.x);
        obstacle.y = game.player.y as f32;
        game.obstacles.obstacles.push(obstacle);

        game.update(0.0, &mut rng);

        assert_eq!(game.lives, 2);
        assert_eq!(game.player.weapon_level, 3);
        assert_eq!(game.scatter_ticks, 99);
        assert_eq!((game.player.x, game.player.y), original_position);
        assert!(!game.obstacles.obstacles[0].active);
        assert_eq!(
            game.pickup_notice
                .as_ref()
                .map(|notice| notice.text.as_str()),
            Some("HIT - LIFE 2")
        );
        assert!(game.score_popups.is_empty());
    }

    #[test]
    fn repair_restores_one_life_up_to_the_initial_maximum() {
        let mut game = playing_game();
        let mut rng = rng();
        game.lives = 2;
        spawn_pickup_on_player(&mut game, PickupKind::Repair);

        game.update(0.0, &mut rng);

        assert_eq!(game.lives, 3);
        assert_eq!(
            game.pickup_notice
                .as_ref()
                .map(|notice| notice.text.as_str()),
            Some("REPAIR - LIFE +1")
        );
    }

    #[test]
    fn repair_at_full_lives_becomes_score() {
        let mut game = playing_game();
        let mut rng = rng();
        spawn_pickup_on_player(&mut game, PickupKind::Repair);

        game.update(0.0, &mut rng);

        assert_eq!(game.lives, MAX_LIVES);
        assert_eq!(game.score, 301); // 300 pickup bonus + 1 survival point
    }

    #[test]
    fn emp_refreshes_duration_and_pause_freezes_it() {
        let mut game = playing_game();
        let mut rng = rng();
        let mut obstacle = Obstacle::new(ObstacleType::Small, 0.0);
        obstacle.y = 10.0;
        game.obstacles.obstacles.push(obstacle);
        spawn_pickup_on_player(&mut game, PickupKind::Emp);
        game.update(0.0, &mut rng);

        assert_eq!(game.emp_ticks, EMP_DURATION_TICKS);
        assert!(
            game.obstacles
                .obstacles
                .iter()
                .all(|obstacle| !obstacle.active)
        );
        assert_eq!(game.score, 1); // clearing a wave grants no reward
        assert_eq!(game.combo, 0);
        assert!(game.score_popups.is_empty());
        assert_eq!(
            game.pickup_notice
                .as_ref()
                .map(|notice| notice.text.as_str()),
            Some("EMP BURST - SCREEN CLEARED")
        );
        assert_eq!(
            game.spawn_interval_multiplier(),
            EMP_SPAWN_INTERVAL_MULTIPLIER
        );

        game.emp_ticks = 20;
        spawn_pickup_on_player(&mut game, PickupKind::Emp);
        game.update(0.0, &mut rng);
        assert_eq!(game.emp_ticks, EMP_DURATION_TICKS);

        game.pause();
        game.update(1.0, &mut rng);
        assert_eq!(game.emp_ticks, EMP_DURATION_TICKS);
    }

    #[test]
    fn pickup_notice_expires_during_play_but_not_pause() {
        let mut game = playing_game();
        let mut rng = rng();
        game.pickup_notice = Some(PickupNotice {
            text: "TEST".to_owned(),
            color: Color::White,
            ticks: 2,
        });

        game.update(0.0, &mut rng);
        assert_eq!(game.pickup_notice.as_ref().unwrap().ticks, 1);
        game.pause();
        game.update(0.0, &mut rng);
        assert_eq!(game.pickup_notice.as_ref().unwrap().ticks, 1);
        game.resume();
        game.update(0.0, &mut rng);
        assert!(game.pickup_notice.is_none());
    }

    #[test]
    fn destroying_an_enemy_releases_its_preassigned_pickup() {
        let mut game = playing_game();
        let mut rng = rng();
        let mut obstacle = Obstacle::new(ObstacleType::Small, 10.0);
        obstacle.y = 10.0;
        obstacle.carried_pickup = Some(PickupKind::Repair);
        game.obstacles.obstacles.push(obstacle);
        game.bullets.bullets.push(Bullet::new(10.0, 10.0, 0.0));

        game.update(0.0, &mut rng);

        assert_eq!(game.pickups.pickups.len(), 1);
        assert_eq!(game.pickups.pickups[0].kind, PickupKind::Repair);
        assert!(game.pickups.pickups[0].active);
        assert!(game.obstacles.obstacles[0].carried_pickup.is_none());
    }

    #[test]
    fn enemy_type_and_combo_determine_kill_score() {
        let mut rng = rng();

        let mut small_game = playing_game();
        shoot_enemy(&mut small_game, ObstacleType::Small);
        small_game.update(0.0, &mut rng);
        assert_eq!(small_game.score, 51); // 50 kill + 1 survival
        assert_eq!(small_game.score_popups[0].score, 50);

        let mut big_game = playing_game();
        shoot_enemy(&mut big_game, ObstacleType::Big);
        big_game.update(0.0, &mut rng);
        assert_eq!(big_game.score, 101); // 100 kill + 1 survival
        assert_eq!(big_game.score_popups[0].score, 100);

        let mut combo_game = playing_game();
        combo_game.combo = 1;
        combo_game.combo_timer = 10;
        shoot_enemy(&mut combo_game, ObstacleType::Big);
        combo_game.update(0.0, &mut rng);
        assert_eq!(combo_game.combo, 2);
        assert_eq!(combo_game.score, 201); // 100 × 2 combo + 1 survival
        assert_eq!(combo_game.score_popups[0].score, 200);
    }

    #[test]
    fn score_popups_expire_during_play_but_freeze_while_paused() {
        let mut game = playing_game();
        let mut rng = rng();
        shoot_enemy(&mut game, ObstacleType::Small);
        game.update(0.0, &mut rng);
        assert_eq!(game.score_popups[0].ticks, SCORE_POPUP_TICKS);

        game.pause();
        game.update(0.0, &mut rng);
        assert_eq!(game.score_popups[0].ticks, SCORE_POPUP_TICKS);

        game.resume();
        game.score_popups[0].ticks = 1;
        game.update(0.0, &mut rng);
        assert!(game.score_popups.is_empty());
    }

    #[test]
    fn debug_mode_survives_starting_a_new_run() {
        let mut game = playing_game();
        game.set_debug_enabled(true);

        game.start();

        assert!(game.debug_enabled);
        assert_eq!(
            game.debug_hud_text(),
            "DEBUG | MODE NORMAL | DIFF 1.00 | SPEED 0.60 | OBS 0 | BUL 0 | PWR 0 | SPAWN 40.0"
        );
    }

    #[test]
    fn returning_to_menu_finalizes_the_current_score() {
        let mut game = playing_game();
        game.score = 4321;

        game.return_to_menu();

        assert_eq!(game.current_high_score(), 4321);
        assert!(matches!(game.state, GameState::Menu));
    }

    #[test]
    fn difficulty_presets_adjust_spawn_interval_and_enemy_speed() {
        let mut game = playing_game();
        game.score = 0;

        game.difficulty_preset = DifficultyPreset::Easy;
        assert_eq!(game.spawn_interval_multiplier(), 1.25);
        assert!((game.enemy_speed_multiplier() - 0.51).abs() < f32::EPSILON);

        game.difficulty_preset = DifficultyPreset::Normal;
        assert_eq!(game.spawn_interval_multiplier(), 1.0);
        assert!((game.enemy_speed_multiplier() - 0.60).abs() < f32::EPSILON);

        game.difficulty_preset = DifficultyPreset::Hard;
        assert_eq!(game.spawn_interval_multiplier(), 0.8);
        assert!((game.enemy_speed_multiplier() - 0.69).abs() < f32::EPSILON);

        game.difficulty_preset = DifficultyPreset::Extreme;
        assert_eq!(game.spawn_interval_multiplier(), 0.65);
        assert!((game.enemy_speed_multiplier() - 0.78).abs() < f32::EPSILON);
    }

    #[test]
    fn switching_difficulty_selects_its_own_high_score() {
        let mut game = playing_game();
        game.return_to_menu();
        let mut scores = HighScores::default();
        scores.set(DifficultyPreset::Easy, 100);
        scores.set(DifficultyPreset::Normal, 200);
        scores.set(DifficultyPreset::Hard, 300);
        scores.set(DifficultyPreset::Extreme, 400);
        game.apply_profile(PlayerProfile {
            selected_difficulty: DifficultyPreset::Normal,
            high_scores: scores,
        });

        assert_eq!(game.current_high_score(), 200);
        game.select_difficulty(DifficultyPreset::Hard);
        assert_eq!(game.current_high_score(), 300);
        game.select_difficulty(DifficultyPreset::Extreme);
        assert_eq!(game.current_high_score(), 400);
    }
}

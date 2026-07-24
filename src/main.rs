mod background;
mod bullet;
mod difficulty;
mod game;
mod obstacle;
mod pickup;
mod player;
mod renderer;
mod score_store;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::Color;
use crossterm::terminal::supports_keyboard_enhancement;
use difficulty::DifficultyPreset;
use game::{Game, GameState};
use player::Dir;
use rand::rng;
use renderer::Renderer;
use score_store::ScoreStore;
use std::io;
use std::thread;
use std::time::{Duration, Instant};

const FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / FPS);

// Reference frame length used to normalize `dt`. All per-frame movement is
// expressed relative to a 30 FPS baseline, so raising `FPS` (or hitting a
// lower effective rate under load) does NOT change real-world speed — a held
// key always travels the same distance per second.
const BASELINE_FRAME: Duration = Duration::from_millis(1000 / 30);

// Without the kitty protocol, a terminal may only send key presses and their
// repeats — no release. The first repeat normally arrives after a few hundred
// milliseconds, while subsequent repeats are much closer together.
const REPEAT_START_MIN: Duration = Duration::from_millis(150);
const REPEAT_START_MAX: Duration = Duration::from_millis(1000);
const DIRECTION_REPEAT_GAP: Duration = Duration::from_millis(250);

const INPUT_USAGE: &str = "Usage: skystrike [--input auto|enhanced|compatible] [--debug]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CliOptions {
    input_mode: RequestedInputMode,
    debug: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RequestedInputMode {
    #[default]
    Auto,
    Enhanced,
    Compatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EffectiveInputMode {
    Enhanced,
    Compatible,
}

impl EffectiveInputMode {
    fn release_events_supported(self) -> bool {
        self == Self::Enhanced
    }

    fn label(self) -> &'static str {
        match self {
            Self::Enhanced => "ENHANCED",
            Self::Compatible => "COMPATIBLE",
        }
    }
}

fn parse_cli_options<I>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let mut options = CliOptions::default();
    let mut input_seen = false;

    while let Some(argument) = args.next() {
        if argument == "--debug" {
            options.debug = true;
            continue;
        }

        let value = if argument == "--input" {
            args.next()
                .ok_or_else(|| format!("missing value for --input\n{INPUT_USAGE}"))?
        } else if let Some(value) = argument.strip_prefix("--input=") {
            value.to_owned()
        } else {
            return Err(format!("unknown argument: {argument}\n{INPUT_USAGE}"));
        };

        if input_seen {
            return Err(format!("duplicate --input argument\n{INPUT_USAGE}"));
        }
        input_seen = true;
        options.input_mode = match value.as_str() {
            "auto" => RequestedInputMode::Auto,
            "enhanced" => RequestedInputMode::Enhanced,
            "compatible" => RequestedInputMode::Compatible,
            _ => return Err(format!("invalid input mode: {value}\n{INPUT_USAGE}")),
        };
    }

    Ok(options)
}

fn resolve_input_mode(requested: RequestedInputMode) -> EffectiveInputMode {
    match requested {
        RequestedInputMode::Enhanced => EffectiveInputMode::Enhanced,
        RequestedInputMode::Compatible => EffectiveInputMode::Compatible,
        RequestedInputMode::Auto => {
            if supports_keyboard_enhancement().unwrap_or(false) {
                EffectiveInputMode::Enhanced
            } else {
                EffectiveInputMode::Compatible
            }
        }
    }
}

/// One input key's state. In fallback mode, an initial press is a single
/// action; only a repeat promotes it to a continuous hold. This prevents a
/// tap from becoming a long movement when a release event is unavailable.
#[derive(Default)]
struct KeyHold {
    active: bool,
    last_event: Option<Instant>,
}

impl KeyHold {
    fn press(&mut self, now: Instant, release_events_supported: bool, is_repeat: bool) {
        let looks_like_repeat = self.last_event.is_some_and(|previous| {
            let elapsed = now.duration_since(previous);
            elapsed >= REPEAT_START_MIN && elapsed <= REPEAT_START_MAX
        });
        self.last_event = Some(now);
        if release_events_supported || is_repeat || looks_like_repeat {
            self.active = true;
        }
    }

    fn release(&mut self) {
        self.active = false;
        self.last_event = None;
    }

    fn expire_after(&mut self, now: Instant, timeout: Duration) {
        if self.active
            && self
                .last_event
                .is_some_and(|last_event| now.duration_since(last_event) > timeout)
        {
            self.release();
        }
    }
}

/// Movement owns held-key state. Fire actions are deliberately independent:
/// J queues one shot, while K toggles frame-driven automatic fire.
struct HeldInput {
    left: KeyHold,
    right: KeyHold,
    up: KeyHold,
    down: KeyHold,
    mode: EffectiveInputMode,
    last_hdir: Option<Dir>,
    last_vdir: Option<Dir>,
    tap_hdir: Option<Dir>,
    tap_vdir: Option<Dir>,
    tap_fire: bool,
    auto_fire: bool,
}

impl HeldInput {
    fn new(mode: EffectiveInputMode) -> Self {
        Self {
            left: KeyHold::default(),
            right: KeyHold::default(),
            up: KeyHold::default(),
            down: KeyHold::default(),
            mode,
            last_hdir: None,
            last_vdir: None,
            tap_hdir: None,
            tap_vdir: None,
            tap_fire: false,
            auto_fire: false,
        }
    }

    fn clear(&mut self) {
        *self = Self::new(self.mode);
    }

    fn clear_movement(&mut self) {
        let auto_fire = self.auto_fire;
        *self = Self::new(self.mode);
        self.auto_fire = auto_fire;
    }

    fn expire_stale(&mut self, now: Instant) {
        // Once the terminal has proven that it reports real key releases,
        // those releases are the source of truth. Expiring from repeat gaps
        // would break chords: while Left+J are held, many terminals repeat
        // only J, but Left is still physically down.
        if self.mode.release_events_supported() {
            return;
        }

        for held in [
            &mut self.left,
            &mut self.right,
            &mut self.up,
            &mut self.down,
        ] {
            held.expire_after(now, DIRECTION_REPEAT_GAP);
        }
    }

    fn h_dir(&self) -> Option<Dir> {
        compute_hdir(self.left.active, self.right.active, self.last_hdir)
    }

    fn v_dir(&self) -> Option<Dir> {
        compute_vdir(self.up.active, self.down.active, self.last_vdir)
    }

    fn clear_taps(&mut self) {
        self.tap_hdir = None;
        self.tap_vdir = None;
        self.tap_fire = false;
    }
}

fn main() -> io::Result<()> {
    let options = match parse_cli_options(std::env::args()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    // Auto queries support before Renderer makes stdout non-blocking. Explicit
    // modes bypass probing so users can override incomplete terminal support.
    let mut effective_mode = resolve_input_mode(options.input_mode);
    let mut renderer = Renderer::new()?;
    renderer.init()?;
    // Ask the terminal to report key press/release/repeat separately so we can
    // stop movement on real key-up instead of relying on OS auto-repeat.
    if effective_mode == EffectiveInputMode::Enhanced && !renderer.enable_kitty(true) {
        effective_mode = EffectiveInputMode::Compatible;
    }

    let mut rng = rng();
    let (w, h) = (renderer.width, renderer.height);
    let mut game = Game::new(w, h, &mut rng);
    game.set_debug_enabled(options.debug);
    let mut score_store = ScoreStore::discover();
    game.apply_profile(score_store.load());

    // Horizontal (left/right) and vertical (up/down) are tracked independently
    // so two axes can be held at once (diagonal move).
    let mut input = HeldInput::new(effective_mode);

    'game_loop: loop {
        // Frame start time, used both to compute dt and to sleep for a
        // steady frame rate.
        let frame_start = Instant::now();
        // Frame-time factor normalized to the 30 FPS baseline. We sleep to
        // ~FRAME_DURATION below, so this is normally ~0.5 at 60 FPS. It is
        // clamped to a sane window: a broken/low-resolution clock that reports
        // zero elapsed time must NOT zero out all motion, so the lower bound
        // keeps the game moving.
        let dt = frame_start.elapsed().as_secs_f32() / BASELINE_FRAME.as_secs_f32();
        let dt = dt.clamp(0.2, 3.0);

        // Input — drain all pending events this frame
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key(key, &mut game, &mut input) {
                        break 'game_loop;
                    }
                }
                Event::Resize(w, h) => {
                    renderer.resize(w, h);
                    game.resize(w, h, &mut rng);
                }
                _ => {}
            }
        }

        // Wait for next frame
        if event::poll(FRAME_DURATION)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key(key, &mut game, &mut input) {
                        break 'game_loop;
                    }
                }
                Event::Resize(w, h) => {
                    renderer.resize(w, h);
                    game.resize(w, h, &mut rng);
                }
                _ => {}
            }
        }

        // Compatible-mode directions expire independently, so a fire action
        // can never prolong stale movement and a direction tap stays a tap.
        input.expire_stale(Instant::now());

        // Apply held-key movement every frame (independent of OS key-repeat).
        // For each axis the most recently pressed direction wins; releasing it
        // falls back to the other still-held direction on that axis.
        let h_dir = input.h_dir().or(input.tap_hdir);
        let v_dir = input.v_dir().or(input.tap_vdir);
        if game.state == GameState::Playing {
            game.player
                .move_in_dir(h_dir, v_dir, game.width, game.height, dt);
            if input.tap_fire || input.auto_fire {
                game.player_fire();
            }
        }
        input.last_hdir = h_dir;
        input.last_vdir = v_dir;
        input.clear_taps();

        // Update. Crossing into GameOver clears movement and auto-fire so a
        // restart always begins from neutral input state.
        let was_playing = game.state == GameState::Playing;
        game.update(dt, &mut rng);
        if was_playing && game.state == GameState::GameOver {
            input.clear();
        }
        let _ = score_store.save_if_changed(game.profile());

        // Render
        renderer.clear();
        game.render(&mut renderer);
        if game.state == GameState::Menu {
            let debug_status = if game.debug_enabled { " | DEBUG" } else { "" };
            let status = format!("INPUT: {}{debug_status}", input.mode.label());
            renderer.put_str(
                1,
                renderer.height.saturating_sub(1),
                &status,
                Color::DarkGrey,
            );
        } else if matches!(game.state, GameState::Playing | GameState::Paused) {
            let hint = "J FIRE | K AUTO: ";
            let state = if input.auto_fire { "ON" } else { "OFF" };
            let suffix = if game.state == GameState::Paused {
                " | P RESUME | ESC MENU"
            } else {
                " | P PAUSE | ESC MENU"
            };
            let total_len = hint.len() + state.len() + suffix.len();
            let x = renderer.width.saturating_sub(total_len as u16 + 2);
            renderer.put_str(x, 1, hint, Color::DarkGrey);
            renderer.put_str(
                x + hint.len() as u16,
                1,
                state,
                if input.auto_fire {
                    Color::Cyan
                } else {
                    Color::DarkGrey
                },
            );
            renderer.put_str(
                x + hint.len() as u16 + state.len() as u16,
                1,
                suffix,
                Color::DarkGrey,
            );
        }
        renderer.flush()?;

        // Keep a steady frame rate: sleep for whatever time remains of this
        // frame. This also guarantees dt is never computed from a zero
        // interval even if the monotonic clock has poor resolution.
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    game.finish_run();
    let _ = score_store.save_if_changed(game.profile());
    renderer.disable_kitty();
    Ok(())
}

/// Horizontal resolution: most-recently-pressed direction wins when both held.
fn compute_hdir(left_held: bool, right_held: bool, last_dir: Option<Dir>) -> Option<Dir> {
    match (left_held, right_held) {
        (true, false) => Some(Dir::Left),
        (false, true) => Some(Dir::Right),
        (true, true) => last_dir.or(Some(Dir::Left)),
        (false, false) => None,
    }
}

/// Vertical resolution: most-recently-pressed direction wins when both held.
fn compute_vdir(up_held: bool, down_held: bool, last_dir: Option<Dir>) -> Option<Dir> {
    match (up_held, down_held) {
        (true, false) => Some(Dir::Up),
        (false, true) => Some(Dir::Down),
        (true, true) => last_dir.or(Some(Dir::Up)),
        (false, false) => None,
    }
}

/// Returns true to quit. Directions maintain tap/hold state, J queues one
/// shot, and K toggles automatic fire independently of movement.
fn handle_key(key: KeyEvent, game: &mut Game, input: &mut HeldInput) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
        _ => {}
    }

    if key.kind == KeyEventKind::Press {
        match (game.state, key.code) {
            (GameState::Playing, KeyCode::Char('p') | KeyCode::Char('P')) => {
                game.pause();
                input.clear_movement();
                return false;
            }
            (GameState::Paused, KeyCode::Char('p') | KeyCode::Char('P')) => {
                game.resume();
                input.clear_movement();
                return false;
            }
            (GameState::Playing | GameState::Paused, KeyCode::Esc) => {
                game.return_to_menu();
                input.clear();
                return false;
            }
            _ => {}
        }
    }

    match game.state {
        GameState::Menu => {
            if key.kind == KeyEventKind::Press {
                let selected = match key.code {
                    KeyCode::Char('1') => Some(DifficultyPreset::Easy),
                    KeyCode::Char('2') => Some(DifficultyPreset::Normal),
                    KeyCode::Char('3') => Some(DifficultyPreset::Hard),
                    KeyCode::Char('4') => Some(DifficultyPreset::Extreme),
                    KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                        Some(game.difficulty_preset.previous())
                    }
                    KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                        Some(game.difficulty_preset.next())
                    }
                    _ => None,
                };
                if let Some(preset) = selected {
                    game.select_difficulty(preset);
                    input.clear_movement();
                } else if key.code == KeyCode::Char(' ') {
                    game.start();
                    input.clear();
                }
            }
        }
        GameState::Playing => match key.code {
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                if key.kind == KeyEventKind::Release {
                    input.left.release();
                } else {
                    let was_active = input.left.active;
                    input.left.press(
                        Instant::now(),
                        input.mode.release_events_supported(),
                        key.kind == KeyEventKind::Repeat,
                    );
                    if !was_active {
                        input.tap_hdir = Some(Dir::Left);
                    }
                    input.last_hdir = Some(Dir::Left);
                }
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                if key.kind == KeyEventKind::Release {
                    input.right.release();
                } else {
                    let was_active = input.right.active;
                    input.right.press(
                        Instant::now(),
                        input.mode.release_events_supported(),
                        key.kind == KeyEventKind::Repeat,
                    );
                    if !was_active {
                        input.tap_hdir = Some(Dir::Right);
                    }
                    input.last_hdir = Some(Dir::Right);
                }
            }
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => {
                if key.kind == KeyEventKind::Release {
                    input.up.release();
                } else {
                    let was_active = input.up.active;
                    input.up.press(
                        Instant::now(),
                        input.mode.release_events_supported(),
                        key.kind == KeyEventKind::Repeat,
                    );
                    if !was_active {
                        input.tap_vdir = Some(Dir::Up);
                    }
                    input.last_vdir = Some(Dir::Up);
                }
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => {
                if key.kind == KeyEventKind::Release {
                    input.down.release();
                } else {
                    let was_active = input.down.active;
                    input.down.press(
                        Instant::now(),
                        input.mode.release_events_supported(),
                        key.kind == KeyEventKind::Repeat,
                    );
                    if !was_active {
                        input.tap_vdir = Some(Dir::Down);
                    }
                    input.last_vdir = Some(Dir::Down);
                }
            }
            KeyCode::Char('j') | KeyCode::Char('J') => {
                if key.kind == KeyEventKind::Press {
                    input.tap_fire = true;
                }
            }
            KeyCode::Char('k') | KeyCode::Char('K') => {
                if key.kind == KeyEventKind::Press {
                    input.auto_fire = !input.auto_fire;
                }
            }
            _ => {}
        },
        GameState::Paused => {}
        GameState::GameOver => {
            if key.code == KeyCode::Char(' ') {
                game.start();
                input.clear();
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn playing_game() -> Game {
        let mut rng = rng();
        let mut game = Game::new(80, 24, &mut rng);
        game.start();
        game
    }

    #[test]
    fn input_mode_defaults_to_auto() {
        assert_eq!(
            parse_cli_options(args(&["skystrike"])),
            Ok(CliOptions::default())
        );
    }

    #[test]
    fn input_mode_accepts_split_and_equals_forms() {
        assert_eq!(
            parse_cli_options(args(&["skystrike", "--input", "enhanced"])),
            Ok(CliOptions {
                input_mode: RequestedInputMode::Enhanced,
                debug: false,
            })
        );
        assert_eq!(
            parse_cli_options(args(&["skystrike", "--input=compatible"])),
            Ok(CliOptions {
                input_mode: RequestedInputMode::Compatible,
                debug: false,
            })
        );
    }

    #[test]
    fn debug_flag_combines_with_input_mode_in_either_order() {
        let expected = CliOptions {
            input_mode: RequestedInputMode::Enhanced,
            debug: true,
        };
        assert_eq!(
            parse_cli_options(args(&["skystrike", "--debug", "--input", "enhanced"])),
            Ok(expected)
        );
        assert_eq!(
            parse_cli_options(args(&["skystrike", "--input=enhanced", "--debug"])),
            Ok(expected)
        );
    }

    #[test]
    fn input_mode_rejects_invalid_values() {
        let error = parse_cli_options(args(&["skystrike", "--input", "unknown"]))
            .expect_err("invalid modes must fail");

        assert!(error.contains(INPUT_USAGE));
    }

    #[test]
    fn difficulty_is_not_a_command_line_option() {
        let error = parse_cli_options(args(&["skystrike", "--difficulty", "hard"]))
            .expect_err("difficulty must be selected in the game menu");

        assert!(error.contains("unknown argument: --difficulty"));
    }

    #[test]
    fn menu_selects_difficulty_on_press_and_ignores_repeat() {
        let mut rng = rng();
        let mut game = Game::new(80, 24, &mut rng);
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );
        assert_eq!(game.difficulty_preset, DifficultyPreset::Hard);

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Repeat),
            &mut game,
            &mut input,
        );
        assert_eq!(game.difficulty_preset, DifficultyPreset::Hard);

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('1'), KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );
        assert_eq!(game.difficulty_preset, DifficultyPreset::Easy);

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('4'), KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );
        assert_eq!(game.difficulty_preset, DifficultyPreset::Extreme);
    }

    #[test]
    fn tap_does_not_become_a_continuous_hold_without_repeat() {
        let now = Instant::now();
        let mut held = KeyHold::default();

        held.press(now, false, false);

        assert!(!held.active);
    }

    #[test]
    fn repeat_promotes_a_key_to_a_continuous_hold() {
        let now = Instant::now();
        let repeat = now
            .checked_add(REPEAT_START_MIN + Duration::from_millis(1))
            .unwrap();
        let mut held = KeyHold::default();

        held.press(now, false, false);
        held.press(repeat, false, true);

        assert!(held.active);
    }

    #[test]
    fn enhanced_direction_remains_held_until_release() {
        let now = Instant::now();
        let later = now
            .checked_add(DIRECTION_REPEAT_GAP + Duration::from_millis(1))
            .unwrap();
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        input.left.press(now, true, false);

        input.expire_stale(later);
        assert!(input.left.active);

        input.left.release();
        assert!(!input.left.active);
    }

    #[test]
    fn clearing_input_preserves_mode_and_disables_auto_fire() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        input.left.active = true;
        input.auto_fire = true;

        input.clear();

        assert_eq!(input.mode, EffectiveInputMode::Enhanced);
        assert!(!input.left.active);
        assert!(!input.auto_fire);
    }

    #[test]
    fn j_press_queues_one_shot_and_repeat_is_ignored() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        let mut game = playing_game();

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );
        assert!(input.tap_fire);

        input.clear_taps();
        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Repeat),
            &mut game,
            &mut input,
        );
        assert!(!input.tap_fire);
    }

    #[test]
    fn k_press_toggles_auto_fire_and_repeat_is_ignored() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        let mut game = playing_game();
        let press =
            KeyEvent::new_with_kind(KeyCode::Char('k'), KeyModifiers::NONE, KeyEventKind::Press);

        handle_key(press, &mut game, &mut input);
        assert!(input.auto_fire);

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('k'), KeyModifiers::NONE, KeyEventKind::Repeat),
            &mut game,
            &mut input,
        );
        assert!(input.auto_fire);

        handle_key(press, &mut game, &mut input);
        assert!(!input.auto_fire);
    }

    #[test]
    fn movement_and_fire_can_be_active_in_the_same_frame() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        let mut game = playing_game();

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );
        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );

        let before_x = game.player.x;
        game.player
            .move_in_dir(input.h_dir(), input.v_dir(), game.width, game.height, 1.0);
        if input.tap_fire {
            game.player_fire();
        }

        assert!(game.player.x > before_x);
        assert!(game.bullets.bullets.iter().any(|bullet| bullet.active));
    }

    #[test]
    fn movement_and_auto_fire_remain_independent() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        let mut game = playing_game();

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Left, KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );
        handle_key(
            KeyEvent::new_with_kind(KeyCode::Char('k'), KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );

        assert!(matches!(input.h_dir(), Some(Dir::Left)));
        assert!(input.auto_fire);
    }

    #[test]
    fn p_toggles_pause_and_clears_movement_but_keeps_auto_fire() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        let mut game = playing_game();
        input.left.active = true;
        input.auto_fire = true;
        let pause =
            KeyEvent::new_with_kind(KeyCode::Char('p'), KeyModifiers::NONE, KeyEventKind::Press);

        handle_key(pause, &mut game, &mut input);
        assert!(matches!(game.state, GameState::Paused));
        assert!(!input.left.active);
        assert!(input.auto_fire);

        handle_key(pause, &mut game, &mut input);
        assert!(matches!(game.state, GameState::Playing));
        assert!(!input.left.active);
        assert!(input.auto_fire);
    }

    #[test]
    fn escape_returns_to_menu_and_clears_auto_fire() {
        let mut input = HeldInput::new(EffectiveInputMode::Enhanced);
        let mut game = playing_game();
        input.auto_fire = true;

        handle_key(
            KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press),
            &mut game,
            &mut input,
        );

        assert!(matches!(game.state, GameState::Menu));
        assert_eq!(game.current_high_score(), game.score);
        assert!(!input.auto_fire);
    }

    #[test]
    fn paused_game_does_not_advance_score() {
        let mut game = playing_game();
        let mut rng = rng();
        game.score = 42;
        game.pause();

        game.update(1.0, &mut rng);

        assert_eq!(game.score, 42);
    }
}

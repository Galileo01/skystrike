# AGENTS.md

Guidance for AI coding agents (Codex / OpenAI-agent tooling, and Claude Code via the `@AGENTS.md` import in `CLAUDE.md`) working in this repository. This is the single source of truth; do not duplicate these rules elsewhere.

## Build & Run

```bash
cargo build              # debug build
cargo build --release    # release build
cargo run                # run in debug
cargo run --release      # run in release
cargo check              # fast type-check (no codegen)
```

## Architecture

Terminal-based tribute to the classic arcade shooter [Raiden (雷电)](https://zh.wikipedia.org/wiki/%E9%9B%B7%E9%9B%B5_(%E9%81%8A%E6%88%B2)) — a player jet at the bottom dodges descending enemy planes. Written in Rust with crossterm for terminal I/O and rand for randomness.

### Game Loop (`main.rs`)
60 FPS fixed-timestep loop (dt-normalized against a 30 FPS baseline so speed is frame-rate independent): drain input events → block for remainder of frame → compute movement direction → update → render. Quit on `q` or `Ctrl+C`. Resize events forwarded to `game.resize()` + `renderer.resize()`. Enables the kitty keyboard protocol (`REPORT_EVENT_TYPES`) so the terminal reports real key press/release/repeat events.

### State Machine (`game.rs`)
Three states: `Menu` (title screen, wait for SPACE) → `Playing` (active gameplay) → `GameOver` (show score, restart on SPACE). Difficulty scales with score: 1.0 base, caps at +2.5 after 2000 points. Collision uses AABB rectangle overlap.

### Components
- **`player.rs`** — Jet fighter at screen bottom, drawn as a 9×7 ASCII sprite. Moves left/right with arrow keys or A/D. `move_speed = 3.0`. Movement is driven by a held-direction model (`Option<Dir>` + `last_dir`), updated every frame in `main.rs`, independent of OS key-repeat.
- **`bullet.rs`** — Player bullets fired with J key. Single cyan `|` projectile per shot, 8-frame cooldown (~4 shots/s). `BulletPool` recycles inactive slots (same object-pool pattern as obstacles).
- **`obstacle.rs`** — Two enemy types: Big (slow, 10×6, DarkRed/Red) and Small (fast, 5×4, DarkYellow/Yellow). `ObstaclePool` manages spawning and recycling (object pool pattern — finds inactive slots before allocating).
- **`background.rs`** — Two-layer scrolling starfield with parallax: far layer (`.`, dim, slow) at width/3 density, near layer (`*`, bright, fast) at width/6 density.
- **`renderer.rs`** — Double-buffered terminal renderer using crossterm. Writes to alternate screen buffer. Flush diffs only changed cells (minimizes cursor moves). Cleanup on Drop restores terminal.

### Key details
- Uses Rust edition 2024
- crossterm for raw-mode input + alternate screen rendering
- rand 0.10 for RNG (uses `rng()` from `rand::rng` and `RngExt`)
- Frame duration: ~16ms (60 FPS); movement uses a `dt` factor normalized to the 30 FPS baseline so real-world speed is frame-rate independent
- Sprite colors per-character via color-mapping functions
- Event loop drains all pending events each frame via `event::poll(Duration::ZERO)`

## Real keyboard input (important pitfalls)

- Raw mode does NOT reliably send key-up (Release) events, so a naive `pressed=true / released=false` boolean makes the player drift forever after one tap.
- We enable `KeyboardEnhancementFlags::REPORT_EVENT_TYPES` (kitty keyboard protocol) so supported terminals report `Press`/`Repeat`/`Release` (`KeyEventKind`).
- `HELD_TIMEOUT = 600ms` safety net: if no key event arrives for that long, held directions are force-cleared. Must exceed the OS key-repeat initial delay (~500ms) so a genuinely held key is not cut off.
- Opposite directions held together: `compute_dir` uses last-pressed wins, falls back to the other still-held direction on release. Do not use simple boolean AND/OR.
- Non-`Playing` states must not call `move_in_dir`, so a held direction does not persist after GameOver.

## Commit message convention

All commits MUST follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<optional scope>): <subject>

<optional body>

<optional footer>
```

- `type`: one of `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`, `style`, `build`, `ci`.
- `subject`: imperative, lowercase, no trailing period, concise.
- `body`: explain *what* and *why* (not how), wrapped ~72 cols, separated from subject by a blank line. Group related changes under bulleted `-` lines when multiple areas are touched.
- Keep code, docs, and config changes coherent: if a commit touches `docs/LEARNING.md` or `docs/ROADMAP.md`, say so in the body.

Examples:
- `fix(input): stop drifting after a single tap via kitty key-release`
- `feat(pickup): add Contra-style power-up pool and drop-on-kill`
- `docs: add ROADMAP and link it from README`

## Commit workflow

Do **NOT** commit on your own after implementing or refactoring a feature. The iterative workflow is:

1. Implement the change and run `cargo build` (and `cargo test` if present) to verify it compiles and is correct.
2. Keep all changes in the working tree (staged or unstaged is fine) and report what was done, so the user can verify the feature (e.g. by running the game).
3. **Wait for the user to verify** the feature before committing. Do not assume a finished task implies a commit.
4. Only commit when the user explicitly asks (e.g. "save to a commit", "commit it"). At that point follow the Commit message convention above (Conventional Commits style).

This applies to Claude, other AI agents, and human contributors.

## Iteration roadmap

Feature planning and the `pickup.rs` (Contra-style power-ups) design live in [`docs/ROADMAP.md`](docs/ROADMAP.md). When implementing a planned item, mark it `[x]` there, add a progress-log line, and keep this file / `docs/LEARNING.md` in sync per the rule below.

## Keeping `docs/LEARNING.md` up to date

`docs/LEARNING.md` is a living learning-notes doc that must stay in sync with the code. **Any code change that alters behavior, architecture, or the player-facing input model MUST update the relevant section of `docs/LEARNING.md` in the same change**, and append a one-line entry under its "修订记录 / Revision log". If the change implements a planned item, also mark it `[x]` and add a line in `docs/ROADMAP.md`.

Checklist before finishing any feature/refactor task:
- Did I touch the game loop, input handling, state machine, rendering, or object pools? → Update the matching section in `docs/LEARNING.md`.
- Did I add/remove/rename a module or public behavior? → Update the structure table and the relevant section.
- Did I change input semantics (keys, hold behavior, kitty protocol, timeouts)? → Update the keyboard-input section and the controls table in both READMEs if needed.
- Append a dated one-line note to the revision log so readers can see what changed and when.
- Writing a commit for this change? Follow the Commit message convention section above.
- Do not leave stale `TODO(后续)` items that the change actually resolves — fill them in instead.

This applies to Claude, other AI agents, and human contributors.

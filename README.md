# Skystrike ⚡

A terminal-based tribute to the classic arcade shooter [**Raiden (雷电)**](https://zh.wikipedia.org/wiki/%E9%9B%B7%E9%9B%BB_(%E9%81%8A%E6%88%B2)), written in Rust. Control a jet fighter and dodge descending enemy planes in a scrolling starfield.

![screenshot](https://img.shields.io/badge/platform-terminal-black)
![Rust](https://img.shields.io/badge/rust-1.92+-orange)

## How to play

| Key | Action |
|---|---|
| ← / → / ↑ / ↓ or W A S D | Move (tap for a step, hold to move continuously) |
| J | Fire one shot |
| K | Toggle auto-fire |
| P | Pause / Resume |
| Esc | Return to menu while playing or paused |
| Space | Start / Restart |
| Q / Ctrl+C | Quit |

Dodge enemy planes and shoot them down with J. Your score increases every frame you survive, and each kill awards 50 points (chained kills within 3s build a combo multiplier, up to `50 × combo`). Destroyed enemies have a 20% chance to drop a reward: `[S]` Scatter upgrades the weapon through 1 / 3 / 5-shot volleys, `[H]` Repair restores one life, and `[E]` EMP immediately clears active enemies before slowing new spawns for 10 seconds; an on-screen notice identifies each pickup. You have 3 lives — a hit keeps your position, removes the colliding enemy, costs one life, and grants ~2s of invincible blink, while weapon progress lasts for the current run. Difficulty ramps over time, and the high score persists in the local application-data directory.

## Build & Run

```bash
# Requires Rust 1.92+
cargo run --release

# Input mode: auto (default), enhanced, or compatible
cargo run --release -- --input auto
cargo run --release -- --input enhanced
cargo run --release -- --input compatible
```

`auto` probes for the Kitty keyboard protocol and falls back to compatible
Press/Repeat-based movement when key-release events are unavailable. Use an
explicit mode to override detection for terminals with partial protocol support.

On macOS, the high score defaults to
`~/Library/Application Support/skystrike/high_score`. Set
`SKYSTRIKE_DATA_DIR` to override the data directory.

## Technical overview

- **60 FPS** fixed-timestep, dt-normalized game loop using crossterm
- Double-buffered terminal rendering — only changed cells are flushed each frame
- Two-layer parallax scrolling starfield background
- Object-pooled enemies, bullets, and pickups (recycles inactive slots before allocating)
- Weighted Scatter / Repair / EMP drops, instant EMP burst, pickup notices, and timed-effect HUD
- Local high-score persistence with missing/corrupt-file fallback
- Two enemy types: heavy bombers (slow, wide) and fighters (fast, narrow)
- AABB collision detection
- Alternate screen buffer — restores your terminal on exit

### Dependencies

- [crossterm](https://github.com/crossterm-rs/crossterm) — terminal I/O
- [rand](https://github.com/rust-random/rand) — RNG


## Documentation

- [Learning notes](docs/LEARNING.md) — what this project teaches: TUI, game loop, state machine, object pools, and the real keyboard-input pitfalls.
- [Roadmap](docs/ROADMAP.md) — feature iteration plan, including the Contra-style power-up (`pickup.rs`) design.

## License

MIT

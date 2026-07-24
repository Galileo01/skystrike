# Skystrike ⚡

A terminal-based tribute to the classic arcade shooter [**Raiden (雷电)**](https://zh.wikipedia.org/wiki/%E9%9B%B7%E9%9B%BB_(%E9%81%8A%E6%88%B2)), written in Rust. Control a jet fighter and dodge descending enemy planes in a scrolling starfield.

![screenshot](https://img.shields.io/badge/platform-terminal-black)
![Rust](https://img.shields.io/badge/rust-1.92+-orange)
[![Crates.io](https://img.shields.io/crates/v/skystrike.svg)](https://crates.io/crates/skystrike)
[![Crates.io Downloads](https://img.shields.io/crates/d/skystrike.svg)](https://crates.io/crates/skystrike)

## How to play

| Key | Action |
|---|---|
| ← / → / ↑ / ↓ or W A S D | Move (tap for a step, hold to move continuously) |
| J | Fire one shot |
| K | Toggle auto-fire |
| P | Pause / Resume |
| Esc | Return to menu while playing or paused |
| 1 / 2 / 3 / 4 or ← / →, A / D (menu) | Select Easy / Normal / Hard / Extreme |
| Space | Start / Restart |
| Q / Ctrl+C | Quit |

Dodge enemy planes and shoot them down with J. Your score increases every frame you survive; small fighters award 50 points and heavy bombers award 100, with a short score popup at the kill position. Chained kills within 3s multiply that enemy's base score. The menu offers Easy / Normal / Hard / Extreme presets; Normal preserves the original balance, while Easy slows enemy speed and spawning and the two higher modes progressively accelerate both. Higher modes also make some top-entry enemies choose an X position near the player's current lane, while preserving random offset and top-lane spacing. Destroyed enemies have a 20% chance to drop a reward: `[S]` Scatter upgrades the weapon through 1 / 3 / 5-shot volleys for 10 seconds, `[H]` Repair restores one life, and `[E]` EMP immediately clears active enemies before slowing new spawns for 10 seconds; an on-screen notice identifies each pickup. A successful Scatter upgrade refreshes its timer, while another pickup at Lv3 becomes 500 score without extending the effect. You have 3 lives — a hit keeps your position, removes the colliding enemy, costs one life, and grants ~2s of invincible blink. The selected difficulty and a separate high score for each preset persist in the local application-data directory.

## Install

SkyStrike 0.1 supports macOS and Linux and requires Rust 1.92 or newer.
The renderer currently uses Unix terminal file-descriptor APIs, so Windows is
not supported yet.

```bash
# From crates.io after the 0.1.0 release
cargo install skystrike
skystrike

# Or install the current source checkout
cargo install --path .
skystrike
```

## Build & Run

```bash
# Requires Rust 1.92+
cargo run --release

# Input mode: auto (default), enhanced, or compatible
cargo run --release -- --input auto
cargo run --release -- --input enhanced
cargo run --release -- --input compatible

# Debug overlay (can be combined with --input)
cargo run --release -- --debug
cargo run --release -- --debug --input compatible
```

`auto` probes for the Kitty keyboard protocol and falls back to compatible
Press/Repeat-based movement when key-release events are unavailable. Use an
explicit mode to override detection for terminals with partial protocol support.

`--debug` reveals each enemy's preassigned drop (`[S]`, `[H]`, `[E]`, or
`[-]`) and shows live difficulty, entity counts, and spawn interval in the HUD.
It changes presentation only; normal play uses the same preassigned rewards.

Settings and per-difficulty records default to
`~/Library/Application Support/skystrike/{settings,high_scores}` on macOS and
`${XDG_DATA_HOME:-~/.local/share}/skystrike/{settings,high_scores}` on Linux.
Set `SKYSTRIKE_DATA_DIR` to override the data directory. A legacy `high_score`
integer is migrated to the Normal record.

## Technical overview

- **60 FPS** fixed-timestep, dt-normalized game loop using crossterm
- Double-buffered terminal rendering — only changed cells are flushed each frame
- Two-layer parallax scrolling starfield background
- Object-pooled enemies, bullets, and pickups (recycles inactive slots before allocating)
- Weighted timed Scatter / Repair / EMP drops, instant EMP burst, pickup notices, and effect countdown HUD
- Persisted menu difficulty and per-preset high scores with legacy migration
- Two enemy types: heavy bombers (slow, wide) and fighters (fast, narrow)
- Difficulty-aware spawn density and top-entry X targeting with overlap fallback
- AABB collision detection
- Alternate screen buffer — restores your terminal on exit

### Dependencies

- [crossterm](https://github.com/crossterm-rs/crossterm) — terminal I/O
- [rand](https://github.com/rust-random/rand) — RNG


## Documentation

- [Learning notes](docs/LEARNING.md) — what this project teaches: TUI, game loop, state machine, object pools, and the real keyboard-input pitfalls.
- [Roadmap](docs/ROADMAP.md) — feature iteration plan, including the Contra-style power-up (`pickup.rs`) design.

## License

[MIT](LICENSE)

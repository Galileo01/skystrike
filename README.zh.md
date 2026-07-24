# Skystrike ⚡

一款在终端中还原经典街机射击游戏 [**雷电**](https://zh.wikipedia.org/wiki/%E9%9B%B7%E9%9B%BB_(%E9%81%8A%E6%88%B2)) 的作品，使用 Rust 编写。操控战斗机在滚动星空中躲避敌方飞机。

![平台](https://img.shields.io/badge/platform-terminal-black)
![Rust](https://img.shields.io/badge/rust-1.92+-orange)
[![Crates.io](https://img.shields.io/crates/v/skystrike.svg)](https://crates.io/crates/skystrike)
[![Crates.io Downloads](https://img.shields.io/crates/d/skystrike.svg)](https://crates.io/crates/skystrike)

## 操作方式

| 按键 | 操作 |
|---|---|
| ← / → / ↑ / ↓ 或 W A S D | 移动(轻点一步,按住持续移动) |
| J | 单次开火 |
| K | 开启 / 关闭自动开火 |
| P | 暂停 / 继续 |
| Esc | 从游戏或暂停返回菜单 |
| 1 / 2 / 3 / 4 或 ← / →、A / D（菜单） | 选择简单 / 一般 / 困难 / 极难 |
| 空格 | 开始 / 重新开始 |
| Q / Ctrl+C | 退出 |

躲避敌机并用 J 键开火击落它们。每存活一帧分数增加；击落小型敌机获得 50 分，击落大型敌机获得 100 分，并在击毁位置短暂显示本次得分。3 秒内连续击杀会用连击倍数乘以对应敌机的基础分。开始菜单提供简单 / 一般 / 困难 / 极难四档；一般保持原有平衡，简单降低敌机速度和生成频率，困难与极难则逐级提高两者。高难度下部分敌机仍从顶部进入，但生成 X 坐标会靠近玩家当前航线，同时保留随机偏移和顶部间距。敌机被击毁时有 20% 概率掉落奖励：`[S]` Scatter 在 10 秒内将武器升级为 1 / 3 / 5 发，`[H]` Repair 恢复一条生命，`[E]` EMP 立即清除当前敌机并在 10 秒内降低生成频率；拾取时画面会显示奖励说明。Scatter 成功升级时会刷新时间，Lv3 再拾取则转换为 500 分但不续时。你有 3 条命——被撞会原地扣 1 命、移除碰撞敌机并获得约 2 秒无敌闪烁。难度选择和每档独立最高分会保存到本机应用数据目录。

## 安装

SkyStrike 0.1 支持 macOS 与 Linux，需要 Rust 1.92 或更高版本。当前渲染器
依赖 Unix 终端文件描述符接口，因此暂不支持 Windows。

```bash
# 0.1.0 发布到 crates.io 后
cargo install skystrike
skystrike

# 或安装当前源码
cargo install --path .
skystrike
```

## 构建与运行

```bash
# 需要 Rust 1.92+
cargo run --release

# 输入模式:auto(默认) / enhanced / compatible
cargo run --release -- --input auto
cargo run --release -- --input enhanced
cargo run --release -- --input compatible

# 调试信息（可以与 --input 组合）
cargo run --release -- --debug
cargo run --release -- --debug --input compatible
```

`auto` 会探测 Kitty 键盘协议；终端不提供松键事件时，自动回退到基于
Press/Repeat 推断的兼容移动。如果终端对协议的实现不完整，可以显式指定模式覆盖探测结果。

`--debug` 会在敌机上方显示其预先确定的奖励（`[S]`、`[H]`、`[E]` 或
`[-]`），并在 HUD 展示实时难度、实体数量和敌机生成间隔。它只增加调试信息，
普通模式与调试模式使用相同的预分配奖励规则。

设置与分档最高分在 macOS 默认保存在
`~/Library/Application Support/skystrike/{settings,high_scores}`，在 Linux
默认保存在 `${XDG_DATA_HOME:-~/.local/share}/skystrike/{settings,high_scores}`。
可以通过 `SKYSTRIKE_DATA_DIR` 覆盖数据目录；旧版单数字 `high_score`
会迁移为一般难度记录。

## 技术概览

- **60 FPS** 固定时间步、dt 归一化的游戏循环，基于 crossterm
- 双缓冲终端渲染——每帧只刷新发生变化的格子
- 两层视差滚动星空背景
- 敌机、子弹与拾取物对象池（回收非活跃槽位后再分配）
- 限时 Scatter / Repair / EMP 加权掉落、EMP 即时清屏、拾取提示与效果倒计时 HUD
- 菜单难度与分档最高分持久化，兼容旧版单数字记录
- 两种敌机类型：重型轰炸机（慢、宽）和战斗机（快、窄）
- 难度驱动的生成密度与顶部 X 轴航线偏置，包含重叠回退
- AABB 碰撞检测
- 备用屏幕缓冲区——退出时自动恢复终端

### 依赖项

- [crossterm](https://github.com/crossterm-rs/crossterm) — 终端 I/O
- [rand](https://github.com/rust-random/rand) — 随机数生成

## 文档

- [学习笔记](docs/LEARNING.md) —— 本项目可学到的内容：TUI、游戏循环、状态机、对象池，以及实际踩过的键盘输入坑。
- [迭代规划](docs/ROADMAP.md) —— 功能迭代清单，含魂斗罗式拾取系统（`pickup.rs`）设计。

## 许可证

[MIT](LICENSE)

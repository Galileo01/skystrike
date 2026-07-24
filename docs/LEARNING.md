# 从 SkyStrike 学习到了什么

> 本文件随功能迭代持续更新。每新增/重构一个能力,请在该能力对应的小节补充要点、坑点与对应源码位置,并在《修订记录》里记一笔。

SkyStrike 是一个用 Rust + crossterm 写的终端版《雷电》射击小游戏。它体量小但覆盖了 TUI、游戏循环、状态机、对象池、真实键盘输入处理等典型主题,适合作为学习项目。

---

## 0. 项目结构速览

| 文件 | 作用 |
| --- | --- |
| `src/main.rs` | 程序入口、`--input` / `--debug` 参数、主循环、键盘分发、kitty 协议与兼容兜底 |
| `src/difficulty.rs` | Easy / Normal / Hard / Extreme 预设、菜单循环顺序、分档最高分与本地 Profile 数据结构 |
| `src/game.rs` | 状态机(`Menu`/`Playing`/`Paused`/`GameOver`)、难度曲线、AABB 碰撞 |
| `src/player.rs` | 玩家飞机精灵、四向移动(水平/垂直两轴)判定 `Dir`、碰撞盒 |
| `src/obstacle.rs` | 敌机(大/小)、预分配奖励、`ObstacleUpdateContext` 与对象池 `ObstaclePool` |
| `src/bullet.rs` | 子弹与对象池 `BulletPool`、共享发射冷却、散射弹道 |
| `src/pickup.rs` | Scatter / Repair / EMP 拾取物与对象池 `PickupPool` |
| `src/score_store.rs` | 难度设置、分档最高分、旧记录迁移与临时文件原子保存 |
| `src/background.rs` | 两层视差星空 |
| `src/renderer.rs` | 双缓冲终端渲染器、diff 刷新、RAII 清理 |

---

## 1. 终端 UI 与 crossterm

- **raw 模式 + 备用屏幕 + 隐藏光标**:任何 TUI 的标配,见 `src/renderer.rs:init()`。
- **RAII 清理**:`Renderer::Drop` 在退出/崩溃时恢复终端(`LeaveAlternateScreen`、显示光标、关闭 raw 模式),避免把用户的 shell 搞乱。
- **安全居中/边界**:用 `cx - len/2`、`saturating_sub` 防止负数与越界,见 `src/game.rs:render_title`。

> TODO(后续):可补充 crossterm 事件模型、颜色与样式 API 的更多用法。

---

## 2. 游戏循环架构

- **固定步长 + dt 化**:`FPS=60`(`src/main.rs`),`FRAME_DURATION` 与逻辑解耦;所有位移(玩家/子弹/敌机/星空)都乘以 `dt`(相对 30FPS 基准的帧时间因子,见 `main.rs` 的 `BASELINE_FRAME`),因此放宽或压低帧率都不会改变真实速度(`src/main.rs`、`src/player.rs`、`src/bullet.rs`、`src/obstacle.rs`、`src/background.rs`)。
- **dt 必须 clamp 兜底,绝不能为 0**:`dt` 由帧间流逝时间算出,而不同运行环境的单调时钟(`Instant`)分辨率差异极大——在某些终端/容器里两帧之间的差值恒为 0。若直接用该差值,`速度 × dt` 全部归零,表现为"背景固定、玩家完全不动、敌机也不动"的假死。正确做法:`dt = frame_start.elapsed().as_secs_f32() / BASELINE.as_secs_f32()`,再 `clamp(0.2, 3.0)` 兜底;并用 `thread::sleep(FRAME_DURATION - elapsed)` 真正卡住每帧时长,保证帧率稳定且 dt 永不为 0(见 `src/main.rs` 主循环)。
- **输入两阶段**:先 `poll(ZERO)` 非阻塞排空已缓存事件,再 `poll(FRAME_DURATION)` 阻塞等下一帧,兼顾响应与省 CPU。
- **每帧顺序**:读输入 → 计算移动方向 → `game.update()` → `clear()`+`render()`+`flush()`。

> 发射冷却(`BulletPool.cooldown`)以帧时间单位递减,使“开火频率”与“帧率”解耦:`J` 请求单发,`K` 切换每帧请求开火,最终都由子弹池冷却限制实际射速。
>
> TODO(后续):若引入可变帧率/插值、或把逻辑与渲染分离,在此更新。

---

## 3. 状态机与模块化

- 四态枚举 `GameState`(`Menu`/`Playing`/`Paused`/`GameOver`,`src/game.rs`),分支清晰;各实体拆成独立模块,Rust 的 `mod` 组织直观。`P` 在 Playing/Paused 之间切换,暂停时 `update()` 在背景更新前直接返回,所以星空、子弹、敌机、分数和计时都冻结;`Esc` 从 Playing/Paused 返回 Menu。
- 状态切换集中在 `start()` / 碰撞触发 `GameOver`,避免散落判断。

---

## 4. 性能与渲染

- **双缓冲 + diff 刷新**:保留 `last_buffer`,`flush()` 只重绘变化的单元格并最小化光标移动(`src/renderer.rs`)。这是 TUI 流畅度的关键。
- **对象池**:`ObstaclePool` / `BulletPool` / `PickupPool` 复用非活跃槽位,避免每帧分配(`src/obstacle.rs`、`src/bullet.rs`、`src/pickup.rs`)。
- **ASCII 精灵画布与碰撞盒必须一致**:敌机每一行都填充到固定奇数宽度,中轴字符位于 `width / 2`;生成边界与 AABB 同样读取该宽度。否则逐行前导空格不一致会让机身或尾部看起来歪斜,调试标签也会与视觉中心错位(`src/obstacle.rs`)。
- 子弹/敌机使用冷却与对象池模式,控制数量上限。

> TODO(后续):可补充 Benmark/ profiling 思路,或 `BufWriter` 批量写 stdout 的收益。

---

## 5. 算法

- **AABB 矩形重叠**做碰撞检测(`rects_overlap`,`src/game.rs`)。
- **视差双层背景**:远层慢、近层快,营造纵深。
- **散射弹道**:每发子弹除向上速度外增加水平速度;Lv2 使用 `-0.35/0/+0.35`,Lv3 使用 `-0.70/-0.35/0/+0.35/+0.70`,形成扇形而非平行弹道。整轮子弹只设置一次公共冷却,因此升级增加覆盖面但不改变开火频率(`src/bullet.rs`)。

---

## 6. 真实键盘输入处理(重点坑)

这是本项目踩过最深的坑,也是最有价值的一节。

### 6.1 系统按键重复 ≠ "按住"
raw 模式下,普通键通常**不发送 key-up(松开)事件**。若用"按下置 true、松开置 false"的布尔法,松手永远不会被感知 → 表现为"按一下就一直走"。

### 6.2 kitty 键盘协议与输入模式
默认 `--input auto` 用 `supports_keyboard_enhancement()` 探测能力；确认支持后启用 `DISAMBIGUATE_ESCAPE_CODES | REPORT_ALL_KEYS_AS_ESCAPE_CODES | REPORT_EVENT_TYPES`,让方向键和 WASD 都以 CSI-u 事件明确报告 `Press` / `Repeat` / `Release`(`KeyEventKind`)。其中 `REPORT_ALL_KEYS_AS_ESCAPE_CODES` 是 WASD 这类普通字符键获得 Repeat/Release 的必要条件。`--input enhanced` 强制使用增强协议,`--input compatible` 则跳过探测并使用 Press/Repeat 兜底。菜单底部会显示最终生效模式。
- 支持:iTerm2、WezTerm、kitty、新版 GNOME Terminal 等。
- 不支持:协商静默失败,不报错,游戏需要兜底。

### 6.3 移动的轻点与长按兜底
增强模式中,`Press` 立即进入按住状态,`Release` 立即停止;兼容模式中,方向的首次 `Press` 只产生一次点按移动,收到 `Repeat`(或可识别的按键重复节奏)后才升级为持续按住,最后一次 Repeat 后 250ms 失效。每个方向独立计时,开火事件不能延长方向状态。

### 6.4 相反方向同时按
用 `Option<Dir>` + `last_dir` 表示"生效方向":两键同按时**最后按下方向优先**;松开当前键后**回落到另一仍按住的方向**(`compute_dir`,`src/main.rs`)。不要用简单布尔与/或,否则会判定成"都不动"。

### 6.5 非游戏状态不移动
`Menu` / `GameOver` 状态下本帧不调用 `move_in_dir`,避免 game over 后方向残留继续滑动。

> TODO(后续):若支持手柄/鼠标或更多键位,在此扩展输入层抽象。

### 6.6 四向移动与独立开火

- `Dir` 从 `Left/Right` 扩展为 `Left/Right/Up/Down`(`src/player.rs`);水平、垂直两轴各自维护按住状态,`move_in_dir` 同时接收两个轴方向,因此可斜向移动。两轴仍遵循“最后按下方向优先、松开回落到另一仍按住方向”的规则(`compute_hdir`/`compute_vdir`)。
- `J` 只在 `Press` 时设置当帧 `tap_fire`,`Repeat` / `Release` 都忽略,因此是单次开火;`K` 只在 `Press` 时切换 `auto_fire`,开启后每帧调用 `player_fire()`,实际射速仍由 `BulletPool` 冷却控制。游戏进行中的右上区域持续显示 `J FIRE | K AUTO: ON/OFF | P PAUSE | ESC MENU`,并用颜色反馈自动开火状态。
- 开火不再持有 `KeyHold`,因此 J/K 不会清除、延长或替换移动方向。进入 GameOver、开始或重开时统一清理移动和自动开火。
- 兼容模式下传统终端可能把系统自动重复也编码为 `Press`,无法从协议层严格区分“再点一次”与“一直按住”;因此 K 的交互契约是**点击切换,不长按**。

### 6.7 dt=0 导致"整个画面不动"(真实环境坑)

- 现象:菜单能进、按键也能收到(诊断日志里 `state` 已切到 `Playing`、`l/r` 标志会变),但星空、敌机、玩家**全部静止**,`dt` 在每一帧都打印为 `0.000`。
- 根因:`dt` 来自 `Instant::now()` 两帧之差。在部分终端/PTY/容器环境里,单调时钟分辨率极差,帧间差恒为 0 → `dt=0` → 所有 `速度 × dt = 0` → 仿真归零。这是把"帧间时间差"裸用作仿真步长、又没有下限兜底的必然风险。
- 修复:`dt` 计算后 `clamp(0.2, 3.0)`,并用 `thread::sleep` 控制每帧时长,使 `dt` 永远 > 0。经验:**任何"乘以时间/帧步长"的物理量都必须有下限**,不能信任底层时钟在任意环境都正常。
- 定位手段:在 `/tmp` 写每帧诊断日志(`state`/`px`/`dt`/按键标志),比反复猜测高效得多——一眼就能区分"循环卡死"还是"仿真步长为 0"。

---

## 7. Rust 语言点

- 枚举 + `Option<Dir>` 建模"无方向/左/右",`match` 做穷尽判定。
- `saturating_sub`/`max`/`min` 做无符号安全边界。
- edition 2024 写法;`Drop` trait 做资源清理。
- `execute!`/`queue!` 对 stdout 的命令缓冲与一次性 flush。

---

## 8. 游戏机制深度(生命值 / 连击)

- **生命值(`lives`)**:初始 3 条。玩家撞敌机不再秒死,而是原地扣 1 命、移除碰撞敌机并进入 `INVINCIBLE_TICKS`(120 tick ≈ 2s)无敌期;命数为 0 才切 `GameOver`。无敌期用**帧计数**(`invincible_ticks`)而非 `dt` 倒计时,复用 Layer 1 的经验——避免再踩“时钟分辨率怪异导致计时归零”的坑(`src/game.rs`)。
- **无敌闪烁**:`Game.render` 在无敌期的奇数 tick 跳过玩家渲染,产生闪烁效果,无需玩家结构感知 `invincible` 状态。
- **类型分值与连击(`combo`)**:`ObstacleType::base_score` 集中定义小型敌机 50 分、大型敌机 100 分;`COMBO_WINDOW`(180 tick ≈ 3s)内连续击杀叠加倍数,最终击杀得分为 `base_score × combo`,窗口内无击杀则 `combo` 清零。同样用**帧计数**(`combo_timer`)实现窗口,而非墙钟时间。
- **局部得分反馈**:击毁敌机后在其中心生成约 0.75 秒的 `ScorePopup`,直接展示应用连击后的实际得分。浮字随游戏暂停而冻结,新一局统一清空;EMP、撞机和越界不会创建浮字(`src/game.rs`)。
- HUD 在 `render_hud` 绘制 `SCORE`(白)、`LV`(黄)、生命 “♥”(红)和武器/Scatter/EMP 状态;Scatter 最后 3 秒变红提示,连击后的实际收益由击毁位置的得分浮字直接反馈。
- 设计取舍:计时类状态一律用帧计数,绝不用 `Instant` 之差——这是本项目踩过的最贵的一个坑(见 6.7)。

## 9. 拾取奖励与武器成长

- 每架敌机生成时就以 20% 概率预先确定 `carried_pickup`;成功后按权重选择 Scatter 55%、Repair 15%、EMP 30%,对应每架敌机约 11%/3%/6%。只有被玩家子弹击毁时才在敌机中心释放该奖励;撞机、越界与 EMP 清屏都会直接丢弃它。把抽签从“死亡时”提前到“生成时”,让调试模式能够观察携带物,但不改变正常模式的掉落概率与触发规则。拾取物以 `0.45 × dt` 下落,越过底边或被玩家碰到后进入非活跃状态,后续掉落优先复用该槽位(`src/obstacle.rs`、`src/pickup.rs`)。
- 玩家武器等级为 Lv1/Lv2/Lv3,对应每轮 1/3/5 发。Scatter 成功升到 Lv2/Lv3 时把 `scatter_ticks` 设为 600 tick(约 10 秒);到期恢复 Lv1 并显示提示。达到 Lv3 后再次拾取不会刷新时间或继续扩张弹幕,而是奖励 500 分;奖励分进入现有难度计算,但不改变连击次数。
- Repair 在生命少于 3 时恢复一条命,满生命时转换为 300 分。EMP 拾取瞬间把当前活跃敌机标记为非活跃(不计分、不增加 combo、不触发掉落),再把敌机生成间隔放大为 1.8 倍并持续 600 tick(约 10 秒);重复拾取刷新持续时间但不叠加倍率。
- 每次拾取设置约 2 秒的 `PickupNotice`,在画面上方显示奖励类型与结果;Scatter 与 EMP 生效期间 HUD 显示剩余秒数。暂停发生在计时更新前,所以提示和两个效果都会冻结;受伤不会立即清除 Scatter,但正常游玩时倒计时继续。
- 撞机后不再传送到下方中央:玩家保留当前位置与 Scatter 剩余时间,碰撞敌机立即回收,并获得约 2 秒无敌。当前玩法没有安全出生区或重生清屏,原地受伤能避免传送进另一架敌机造成连续扣命;`Game::start` 仍会创建新玩家并清零 `scatter_ticks`,所以新一局恢复 Lv1。
- 暂停在所有实体更新前返回,所以拾取物与子弹、敌机、星空一起冻结;Paused 仍渲染拾取物,恢复后从原位置继续。

## 10. 调试模式

- 命令行解析集中到 `CliOptions`,所以 `--debug` 可以单独使用,也可以与 `--input auto|enhanced|compatible` 按任意顺序组合。菜单底部追加 `DEBUG`,避免忘记当前启动方式(`src/main.rs`)。
- 调试模式在每架敌机上方显示预分配奖励:`[S]`、`[H]`、`[E]` 或无奖励 `[-]`;顶部额外显示难度倍率、活跃敌机/子弹/拾取物数量和当前生成间隔(`src/obstacle.rs`、`src/game.rs`)。
- `debug_enabled` 只控制附加渲染,不参与随机抽取、碰撞或得分逻辑。普通模式也会在生成敌机时预分配奖励,因此开关调试模式不会形成两套玩法。可复现随机局面的 `--seed` 留给后续迭代。

## 11. 菜单难度预设

- `DifficultyPreset` 集中定义四档规则:Easy 的敌机生成间隔 ×1.25、速度 ×0.85;Normal 均为 ×1.0;Hard 为 ×0.8 / ×1.15;Extreme 为 ×0.65 / ×1.30。它们叠加在原有分数曲线上,不改变生命、奖励概率、敌机分值或连击窗口(`src/difficulty.rs`、`src/game.rs`)。
- **密度倍率必须允许小于 1**:`ObstaclePool::set_spawn_interval_multiplier` 过去为 EMP 抑制而用 `max(1.0)`,接入 Hard/Extreme 后会把 0.8/0.65 悄悄抬回 1.0,造成“配置变密、运行未生效”。现在只保留 0.1 的安全下限,让高难缩短间隔、EMP 再乘 1.8 延长间隔(`src/obstacle.rs`)。
- **只偏置 X,不改变顶部 Y**:敌机仍以 `y = -height` 创建并逐行进入。Easy 全随机;Normal/Hard/Extreme 分别以 10%/30%/50% 概率把敌机中心放到玩家当前中心 X 的 ±16/±12/±8 列内。候选位置会 clamp 到屏幕,若与仍在顶部区域的敌机及 3 列安全间距重叠,最多尝试 6 个普通随机位置回退(`src/difficulty.rs`、`src/obstacle.rs`)。
- 终端尺寸、速度/难度倍率、玩家中心 X、预设与 `dt` 统一装入 `ObstacleUpdateContext`,避免 `ObstaclePool::update` 随生成策略扩展成难以维护的长参数列表。
- 难度属于玩家玩法选择,因此不提供命令行参数。菜单用 `1/2/3/4` 直接选择,也支持 `A/D` 和左右方向键循环切换;只处理 `Press`,忽略增强协议的 `Repeat`,开始游戏时继续清空移动状态(`src/main.rs`)。
- 首次运行或设置损坏时默认 Normal;切换后主循环把 `PlayerProfile` 写回本地,下次启动恢复上次档位。HUD 显示档位和等级,调试 HUD 额外显示档位、实际速度倍率与生成间隔。

## 12. 设置与分档最高分持久化

- `Game::finish_run` 在 GameOver、Esc 返回菜单和正常退出时只更新当前档位记录;四档分开比较,避免简单模式成绩覆盖高难模式。主循环通过 `ScoreStore::save_if_changed` 只在档位或成绩变化时写盘。
- 同一应用数据目录下用 `settings` 保存 `difficulty=normal`,用 `high_scores` 保存 `easy/normal/hard/extreme` 四行记录。macOS 目录为 `~/Library/Application Support/skystrike`,Linux 为 `$XDG_DATA_HOME/skystrike`(或 `~/.local/share/skystrike`),Windows 为 `%LOCALAPPDATA%/skystrike`;`SKYSTRIKE_DATA_DIR` 仍可覆盖目录。
- 若新 `high_scores` 不存在但旧 `high_score` 是有效整数,读取时把它迁移为 Normal 记录;其他档位从 0 开始。已有三行 `high_scores` 缺少 Extreme 时也安全回退为 0。缺失、不可读或损坏字段安全回退,写入仍采用临时文件 + rename(`src/score_store.rs`)。

## 13. 构建、打包与发布边界

- `Cargo.toml` 用 `rust-version = "1.92"` 固定首版 MSRV,并补齐 crates.io 展示所需的 description、MIT license、repository、readme、keywords 与 categories。
- 发布包采用 `include` 白名单,只包含编译所需源码、双语 README、LICENSE、CHANGELOG 和项目文档;`AGENTS.md`、`CLAUDE.md` 等协作文件不会进入 `.crate`。每次发布前用 `cargo package --list` 审核实际清单。
- 当前 `renderer.rs` 使用 Unix 的 `AsRawFd`、`fcntl` 与 `O_NONBLOCK`,所以 `0.1.0` 明确支持 macOS/Linux,暂不承诺 Windows。GitHub Actions 在两个系统固定 Rust 1.92,执行格式、测试、release 构建、Clippy 与 package 验证。
- crates.io 版本不可覆盖。流程必须是 package/dry-run → 从产物独立安装 → 用户确认 → release commit/tag → `cargo publish`;上传与 tag 不在普通功能迭代中自动执行。

```bash
cargo build              # debug 构建
cargo build --release    # release 构建
cargo run                # 运行(debug)
cargo run --release      # 运行(release)
cargo run -- --input auto        # 自动探测(默认)
cargo run -- --input enhanced    # 强制增强键盘协议
cargo run -- --input compatible  # 强制传统终端兜底
cargo run -- --debug             # 显示敌机奖励与实时调试 HUD
cargo check              # 仅类型检查
cargo package --list     # 审核将进入 crates.io 的文件
cargo publish --dry-run  # 只做发布演练,不上传
```

---


- 2026-07-09:Layer 1 完成——四向移动(WASD/方向键)、FPS 提升至 60、全量 dt 化(位移与开火冷却均按 `dt` 缩放)、完成初版射击手感;更新第 2、6.3、6.6 节与结构表。
- 2026-07-10:修复 dt=0 导致全静止——`dt` 改由 `frame_start.elapsed()` 计算并 `clamp(0.2,3.0)` 兜底,主循环用 `thread::sleep` 控帧率;另修复退出卡死(`'game_loop` 标签)、渲染非阻塞写(O_NONBLOCK)、菜单星空滚动、开始/重开不依赖 `key.kind`、重开清空按住状态。更新第 2、6.6、6.7 节。
- 2026-07-10:Layer 2 启动——生命值/血条(3 命 + 无敌闪烁 + 重置出生点)、得分与连击倍数(3s 窗口,`50×combo`,HUD 显示 `COMBO xN`);新增第 8 节说明计时类状态一律用帧计数而非墙钟。
## 修订记录

- 2026-07-24:完成 `0.1.0` 发布准备演练——补齐 Cargo 元数据、MIT LICENSE、CHANGELOG、crates.io 徽章与 package 白名单,明确 macOS/Linux 边界并增加双平台 CI;dry-run 及从 `.crate` 独立安装验证通过。
- 2026-07-24:Scatter 从整局永久升级改为 10 秒限时强化——成功升级刷新,满级拾取不续时,HUD 显示倒计时且最后 3 秒变红;暂停冻结、受伤保留,到期恢复 Lv1。
- 2026-07-24:难度接入敌机生成密度与 X 轴航线偏置——Y 入场不变,高难按概率靠近玩家当前 X,顶部重叠时随机回退;修复 Hard/Extreme 小于 1 的间隔倍率被钳制失效。
- 2026-07-23:新增仅菜单可选的 Easy/Normal/Hard/Extreme 难度预设,选择跨启动持久化;最高分改为按档保存,旧单数字记录迁移到 Normal,HUD/调试 HUD 与测试同步更新。
- 2026-07-22:区分敌机击杀价值——小型 50 分、大型 100 分,再乘连击倍数;击毁位置短暂显示实际得分,补充类型计分、连击和浮字生命周期测试。
- 2026-07-22:修正大、小敌机 ASCII 精灵的固定宽度与纵向中轴,使造型、奖励标签、生成边界和碰撞盒使用同一中心;增加精灵宽度/中轴回归测试。
- 2026-07-21:新增 `--debug` 调试模式——敌机生成时预分配奖励并显示 `S/H/E/-`,HUD 展示难度、实体数量与生成间隔;调试开关只影响展示,不改变掉落规则。
- 2026-07-20:撤销实验性 EMP 全屏闪光及其渲染器改造;终端全局背景色、清屏和非阻塞 diff 组合在不同终端上会产生残色与半帧,恢复稳定的逐格 diff 渲染,保留即时清敌与文字提示。
- 2026-07-16:优化受伤与 EMP 反馈——撞机改为原地扣命并移除碰撞敌机,显示剩余生命;EMP 拾取时先无奖励清除当前敌机,再维持 10 秒生成抑制。
- 2026-07-15:扩展拾取系统——新增 Repair/EMP 加权掉落、拾取结果提示、EMP 剩余时间 HUD 与生成间隔抑制;新增跨平台最高分容错加载和正常结束时原子保存。
- 2026-07-14:完成 Scatter 拾取闭环——新增 `PickupPool`、20% 击杀掉落、玩家拾取碰撞、Lv1/Lv2/Lv3 的 1/3/5 发扇形弹道、满级 500 分奖励与 HUD 武器等级;重生保留等级,新一局重置。
- 2026-07-13:重构键盘状态——启用完整 kitty CSI-u flags，使普通字符键 J 也报告 Repeat/Release；无 Release 终端首次 Press 只触发一次、Repeat 才提升为持续状态；修复方向 + J 组合键互相中断。更新第 6.2、6.3、6.6 节并新增组合行为回归测试。
- 2026-07-14:定稿跨终端输入——新增 `auto/enhanced/compatible` 启动模式,`J` 改为单发,`K` 点击切换自动开火,移动保留轻点和长按;HUD 显示 J/K/P/Esc 提示与自动开火状态,新增 Paused 状态及 P 暂停/Esc 返回菜单,并修正菜单 ASCII 标题。
- 初版:整理项目结构、TUI/游戏循环/状态机/对象池/键盘输入处理(含 kitty 协议与安全超时)等学习要点。
- 迭代规划见 `docs/ROADMAP.md`:分层路线图(手感/玩法/工程),含魂斗罗式拾取系统 `pickup.rs` 的设计与接入点。

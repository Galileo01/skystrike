# SkyStrike 迭代规划 / Roadmap

> 本文件随开发推进持续更新。每完成一项,把对应条目标记为 `[x]` 并在末尾「进度记录」记一笔。
> 设计原则:复用现有对象池 / 状态机 / `rects_overlap` 碰撞,不推翻架构。

当前代码骨架已跑通:玩家左右移动(按住连续)、敌机下落、子弹击杀、AABB 碰撞、双缓冲渲染。本规划按"投入小→反馈快→玩法深"分层。

---

## Layer 1 — 手感与体验(优先)

- [x] **垂直移动**:玩家加 `Up/Down`(或 `W/S`)。扩展 `Dir` 为四向(`Left/Right/Up/Down`),`move_in_dir` 改为接收水平/垂直两个方向,支持斜向移动。
- [x] **帧率提升**:`FPS` 30 → 60(`src/main.rs`)。
- [x] **dt 化移动**:移动/子弹/敌机/星空全部按 `dt`(相对 30FPS 基准的帧时间因子)缩放,`move_speed` 量纲统一为每基准帧像素,帧率变化不再导致速度漂移。
- [x] **射击手感**:`J` 单次开火,`K` 点击切换自动开火;开火状态与移动按住状态解耦,`BulletPool` 冷却以帧时间单位递减,发射频率与帧率解耦。
- [x] **暂停 / 返回菜单**:`P` 冻结/恢复整个游戏世界,`Esc` 从游戏或暂停返回开始菜单,切换时清理移动按住状态。

## Layer 2 — 玩法深度(让游戏有目标)

- [x] **生命值 / 血条**:`lives`(3 条),撞击后原地扣 1 命、移除碰撞敌机并进入 ~2s 无敌闪烁;命数为 0 才 `GameOver`(`src/game.rs`)。
- [x] **得分与连击**:连续击杀(3s 窗口内)叠加 `combo`;小型敌机基础分 50、大型敌机 100,结算为 `base_score × combo`,击毁位置显示实际得分(`src/game.rs`)。
- [x] **菜单难度选择**:Easy / Normal / Hard / Extreme 只在游戏菜单选择;默认 Normal,选择持久化。预设分别调整敌机生成间隔与移动速度,不改变生命、奖励和计分规则。
- [x] **难度化生成位置**:Y 轴保持从屏幕顶部逐行进入;Normal/Hard/Extreme 分别以 10%/30%/50% 概率在玩家当前 X 附近生成,偏移半径逐级收紧,顶部占用时回退到普通随机。
- [x] **最高分持久化**:四档难度分别保存最高分;旧版单数字记录迁移为 Normal,缺失/损坏文件安全回退。
- [x] **Scatter 拾取闭环 `pickup.rs`**:敌机概率掉落、对象池、玩家拾取碰撞、Lv1/Lv2/Lv3 对应 1/3/5 发扇形弹道、10 秒倒计时、HUD 状态与满级奖励。
- [x] **首批扩展拾取**:加入 Repair(恢复生命)与 EMP(即时清屏、10 秒降低生成频率),并显示拾取结果和限时效果 HUD。
- [ ] **后续拾取类型**:继续加入 Rapid、BigBullet、Shield。
- [ ] **敌机开火 / 弹幕**:`Big` 类型偶尔向下发子弹,复用 `BulletPool`(需区分敌我阵营/颜色)。难度关键放大器。
- [ ] **Boss / 波次**:`ObstacleType` 加 `Boss` 变体;`game.update` 按分数进入新波次。

## Layer 3 — 工程健壮性(长期维护)

- [x] **调试模式 MVP**:支持 `--debug`,敌机生成时预先确定携带奖励并显示 `S/H/E/-`;调试 HUD 展示难度、活跃敌机/子弹/奖励数量与生成间隔。
- [ ] **可复现随机局面**:增加 `--seed <数字>`,便于稳定重放指定敌机、奖励和生成序列。
- [ ] **CI 卡点**:`src/` 改动但 `docs/LEARNING.md` 修订记录未更新则失败,把文档同步约束变硬强制。
- [ ] **单元测试**:`rects_overlap`、`compute_dir`、`ObstaclePool`/`BulletPool` 回收等纯逻辑加 `cargo test`。
- [ ] **音效(feature 可选)**:`rodio`/`cpal`,不破坏纯文本 TUI 定位。
- [ ] **配置化**:`FPS`/`move_speed`/`HELD_TIMEOUT`/难度曲线抽到 `config.rs`/TOML。

---

## 发布与分发规划

发布采用 **crates.io + GitHub Releases 双渠道**:crates.io 面向已有 Rust 工具链的用户,提供 `cargo install skystrike`;GitHub Releases 后续提供预编译二进制,降低普通玩家的安装门槛。`0.1.0` 已发布到 crates.io,后续版本由 Release Please 管理版本号、CHANGELOG、tag 与 GitHub Release。

### 发布前检查

1. [x] **完成调试模式 MVP**:`--debug` + 敌机奖励标记 + 调试 HUD。
2. [ ] **增加可复现随机局面**:支持 `--seed <数字>`,让调试结果能够稳定重放。
3. [x] **明确平台范围**:README 标注首版支持 macOS/Linux;渲染器依赖 Unix `AsRawFd` / `fcntl`,Windows 暂不支持。
4. [x] **增加验证 CI**:macOS、Linux 固定 Rust 1.92,执行 fmt、test、release build、Clippy 与 `cargo package`。
5. [x] **补齐 Cargo 元数据**:`description`、MIT `license`、`repository`、`readme`、`keywords`、`categories` 与 `rust-version = 1.92`。
6. [x] **精简 crate 内容**:用 `include` 只发布源码、双语 README、LICENSE、CHANGELOG 和项目文档,排除 `AGENTS.md`、`CLAUDE.md`。
7. [x] **发布前演练**:`cargo publish --dry-run` 通过;生成的 20 文件 `.crate` 可独立编译并安装运行,`CHANGELOG.md` 已准备。最终确认后再创建 Git tag。
8. [x] **接入受控发布 workflow**:`0.1.0` 已通过 main/版本校验、完整 dry-run 与 `crates-io` Environment 审批发布。
9. [x] **发布 crates.io `0.1.0`**:`skystrike 0.1.0` 已上线,`cargo install skystrike` 可直接安装。
10. [ ] **发布 GitHub Releases**:提供 macOS、Linux 预编译包、校验值和简短安装说明;Windows 支持完成后再增加对应产物。
11. [ ] **验证 Release Please 首轮自动发布**:以 `0.1.0` 为基线自动维护 Release PR;合并后创建 tag/GitHub Release,经完整校验和 `crates-io` Environment 审批发布下一版本。

### 发布原则

- crates.io 版本不可覆盖,首次正式发布前必须完成 dry-run 和安装验证。
- crates.io 是 Rust 用户的安装渠道,不替代面向普通玩家的预编译包。
- 发布流程不与玩法迭代混在同一个 commit;元数据/CI、版本号/tag、发布产物分别保持可审计。

---

## 拾取系统专章(PowerUp / 魂斗罗式)

### 设计要点

- 新模块 `pickup.rs`:`Pickup { kind, x, y, active, color, symbol }` + `PickupPool`(同 `ObstaclePool` 对象池模式)。
- 掉落来源:**敌机被击杀时按 20% 概率掉落**,契合魂斗罗手感。
- 触碰判定:复用 `rects_overlap`,在"玩家 vs 障碍"循环旁处理"玩家 vs 拾取物"。

### 两类增益(按你定)

- **已完成——Scatter 限时进化**:
  - 三级武器,分别发射 1/3/5 发扇形弹道。
  - 成功升到 Lv2/Lv3 时获得并刷新 10 秒持续时间;暂停冻结,受伤不立即清除。
  - 倒计时结束恢复 Lv1;新一局也恢复 Lv1。
  - 满级再次拾取奖励 500 分,但不刷新持续时间。
- **已完成——首批扩展**:
  - `Repair`:生命不足 3 时恢复 1 条,满生命转 300 分。
  - `EMP`:立即无奖励清除当前敌机,随后 10 秒内敌机生成间隔 ×1.8;重复拾取刷新时间。
  - 总掉落率 20%;掉落成功后 Scatter/Repair/EMP 权重为 55%/15%/30%。
  - 拾取时显示约 2 秒结果提示,EMP 在 HUD 显示剩余时间。
- **后续扩展**:
  - `Rapid`:射速提升。
  - `BigBullet`:限时大子弹。
  - `Shield`:限时免伤或免死一次。
  - 不再规划 `AutoFire` 拾取——`K` 已提供玩家可控的自动开火。

### 接入点

- `Player`:保存当前 `weapon_level`;`Game::scatter_ticks` 独立管理 Scatter 生命周期。
- `BulletPool.fire`:已按 `weapon_level` 生成一轮多发子弹并共享冷却;BigBullet 后续扩展尺寸与渲染。
- `game.update`:已接入掉落与拾取碰撞;后续在同一阶段更新限时增益。
- HUD:已显示武器等级和 Scatter / EMP 剩余秒数,Scatter 最后 3 秒变红提醒。

### 建议落地顺序

1. [x] `pickup.rs` + 敌杀掉落 + Scatter 三级升级。
2. [x] Repair / EMP、拾取提示与限时效果 HUD。
3. [ ] Rapid / BigBullet / Shield 扩展。
4. [ ] 根据试玩数据调整掉落率、权重、EMP 时长与弹道角度。

### 已定规则

- 敌杀掉落,概率 20%。
- 掉落成功后的权重:Scatter 55%、Repair 15%、EMP 30%。
- Scatter 受伤不立即掉级,但倒计时继续;暂停冻结,新一局重置。
- 生命值与 HUD 已在 Layer 2 前序迭代完成。

---


- Layer 1 完成(2026-07-09):垂直移动(四向 + WASD)、FPS 30→60、全量 dt 化、开火冷却 dt 化。
- 健壮性修复(2026-07-10~13):dt=0 兜底(`clamp` + `thread::sleep` 控帧率)、退出卡死修复(`'game_loop` 标签)、渲染非阻塞写(`O_NONBLOCK`)、菜单星空滚动、开始/重开清空输入状态;新增 `auto/enhanced/compatible` 输入模式,移动支持轻点和长按,`J` 单发与 `K` 自动开火完全独立于移动。Layer 1 功能项保持不变(均已 `[x]`)。
- Layer 2 开始(2026-07-10):生命值/血条(3 命 + 无敌闪烁 + 重置)、得分与连击倍数(3s 窗口,`50×combo`)。
## 进度记录

- 2026-07-24:以 crates.io `0.1.0` 为基线接入 Release Please——main 提交自动维护 Release PR,合并后创建 tag/GitHub Release并发布下一 crate 版本;README 徽章更换缓存键,避免继续显示发布前的 `not found`。
- 2026-07-24:新增 crates.io 受控发布 workflow——只允许手动从 main 发起,输入版本必须匹配 Cargo.toml;无权限 job 先完成全量验证/dry-run,再进入 `crates-io` Environment 人工审批并读取 Token 发布。
- 2026-07-24:完成 `0.1.0` 发布演练——补齐 crate 元数据、MIT LICENSE、CHANGELOG 与 crates.io 徽章,明确 macOS/Linux 平台范围并增加双平台 CI;20 文件 package/dry-run/独立安装均通过。
- 2026-07-24:Scatter 改为 10 秒限时强化——升级刷新时间,满级拾取只转 500 分且不续时,HUD 倒计时并在到期后恢复 Lv1;暂停冻结、受伤保留剩余时间。
- 2026-07-24:难度开始影响敌机 X 轴生成——保持顶部 Y 入场,高难模式按概率偏向玩家当前航线并保留随机偏移/重叠回退;修复小于 1.0 的生成间隔倍率被错误钳制,Hard/Extreme 密度现已真实生效。
- 2026-07-23:完成菜单难度选择并扩展 Extreme——Easy/Normal/Hard/Extreme 分别应用生成间隔与敌机速度倍率,选择写入本地 `settings`,最高分按档写入 `high_scores`,旧 `high_score` 迁移为 Normal。
- 2026-07-22:完成敌机类型计分差异——小型 50、大型 100,沿用连击乘数并在击毁位置显示实际得分;大型敌机多段生命留待试玩后决定。
- 2026-07-21:完成调试模式 MVP——新增 `--debug`,敌机显示预分配的 `S/H/E/-` 奖励,HUD 展示难度、实体数量和生成间隔;`--seed` 留作下一小步。
- 2026-07-21:新增发布与分发规划——调试模式后补齐平台 CI、Cargo 元数据和打包审计,先 dry-run,再以 crates.io + GitHub Releases 双渠道发布 `0.1.0`。
- 2026-07-20:撤销实验性 EMP 全屏闪光与配套渲染改造;跨终端残色和半帧风险高于视觉收益,恢复稳定 diff 渲染,EMP 保留即时清屏、提示和 10 秒生成抑制。
- 2026-07-16:优化受伤与 EMP 体验——受伤保留玩家位置、移除碰撞敌机并提示剩余生命;EMP 增加即时无奖励清屏,使效果可感知。
- 2026-07-15:新增 Repair/EMP 加权掉落、拾取结果提示与 EMP 剩余时间 HUD;EMP 将生成间隔放大 1.8 倍并持续约 10 秒。最高分在 GameOver、返回菜单和正常退出时保存到系统应用数据目录。
- 2026-07-14:完成 Scatter 拾取 MVP——20% 击杀掉落、对象池、拾取升级、1/3/5 发扇形弹道、HUD 等级与满级 500 分奖励;移除已被 K 开关取代的 AutoFire 拾取规划。
- 2026-07-14:完成跨终端输入定稿——可选自动/增强/兼容模式,`J` 单发,`K` 切换自动开火,`P` 暂停,`Esc` 返回菜单,移动保留轻点与长按,HUD 显示开火与暂停提示。
- 初版规划:分层(手感 / 玩法 / 工程)、拾取系统专章与接入点、待拍板项。

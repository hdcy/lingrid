# SMAPI 安装：实现与验证

本次实现需求池中的 SMAPI-01 / SMAPI-02：检测游戏目录中的 SMAPI，把官方发行包安装到该目录，支持更新、彻底卸载，以及在管理器内查看游戏运行日志。没有实现 Mod 扫描、Mod 安装或游戏启动。

## 结论先行

- **安装来源**：GitHub Releases 上 `Pathoschild/SMAPI` 的最新稳定发行包（`SMAPI-x.y.z-installer.zip`）。
- **校验**：下载后按 GitHub API 返回的 `digest` 字段做 SHA-256 校验；不匹配、或发行版没有公布摘要，都中止且不写入游戏目录。
- **安装方式**：解包官方安装器里的负载（`internal/windows|linux/install.dat`）后自行落盘，**不启动官方 `SMAPI.Installer` 进程**，因此目标机器不需要 .NET 运行时。
- **额外能力**：官方安装器没有事务与回滚，本实现补上了安装前备份、失败回滚和彻底卸载；每一步都向界面报告进度，并可读取游戏运行日志。

## 结构与职责

```text
src-tauri/src/smapi/
  model.rs      结果类型：SmapiRelease、SmapiStatus、SmapiInstallOutcome、SmapiUninstallReport
  release.rs    GitHub 发布信息查询、下载、SHA-256 校验与缓存
  archive.rs    zip 读取与解包（拒绝绝对路径、.. 穿越与符号链接）
  install.rs    安装、卸载、检测、启动器替换与回滚
  logs.rs       定位并读取游戏目录之外的 SMAPI 运行日志
  records.rs    安装记录：版本号与安装时新建的路径（原子写入）
  progress.rs   步骤与字节进度事件
  commands.rs   Tauri 命令、步骤编排与阻塞任务调度
  tests.rs      临时目录测试，以及两个可选的实测入口
src/features/smapi/
  types.ts      IPC 数据类型、运行时校验与步骤合并
  api.ts        命令调用
  store.ts      状态、检查更新、安装、卸载、日志与进度事件订阅
  SmapiPanel.vue 游戏目录页中的 SMAPI 面板（步骤日志、下载进度、卸载确认、游戏日志）
```

页面不读文件、不拼路径。全部文件访问在 `spawn_blocking` 中执行。

## 安装流程

与官方 `InteractiveInstaller` 的安装步骤对齐，每一步都会上报进度：

1. `release`：查询最新稳定发行版。
2. `download`：下载到缓存目录（已有缓存则跳过，标记为 `cached`）；期间持续上报已下载字节数与总量。
3. `verify`：按 `digest` 校验 SHA-256；缓存里的文件同样重新校验，被截断或改动会被拒绝。发行版没有 `digest` 时报 `smapiDigestMissing` 并中止（`Pathoschild/SMAPI` 4.x 起都会提供）。
4. `extract`：从安装器包中取出本平台负载并解包到临时目录。
5. `install`：
   - 校验目标确实是游戏目录（复用目录校验），并要求存在 `Stardew Valley.deps.json`；
   - 读取（保留）`smapi-internal/config.user.json` 中的用户设置；
   - 把本次可能改动的每个路径**移动**到游戏目录内的临时目录（不复制，同盘 rename 是原子且不占额外空间），用于失败回滚；
   - 记录**本次新建的路径**（安装前不存在的相对路径），供卸载时精确清理；
   - 清理上一次安装写入的文件（更新场景）；
   - 复制负载顶层条目到游戏目录（`Mods` 单独处理，`mcs` 为 macOS 符号链接，跳过）；
   - **仅 Linux**：`StardewValley` 备份为 `StardewValley-original`，`unix-launcher.sh` 就位为 `StardewValley`，并给 `StardewValley` 与 `StardewModdingAPI` 置可执行位（zip 不保留权限位）；
   - 由游戏自带的 `Stardew Valley.deps.json` 生成 `StardewModdingAPI.deps.json`（SMAPI 靠它解析 `libSkiaSharp` 等原生库）；
   - 创建 `Mods` 目录，并按唯一 ID 白名单（`SMAPI.SaveBackup`、`SMAPI.ConsoleCommands`）安装捆绑 Mod；清单字段按 `UniqueId` / `UniqueID` 两种拼写读取。

## 卸载流程

卸载（`uninstall` 步骤）分四层清理，范围由窄到宽：

1. **恢复启动器**：Linux 上把 `StardewValley-original` 换回 `StardewValley`。
2. **本实现管理的文件**：`StardewModdingAPI*`、`smapi-internal/`、`Mods/.cache` 等。
3. **捆绑 Mod**：按 manifest 的唯一 ID 识别 `Mods/SaveBackup`、`Mods/ConsoleCommands` 后删除 —— 用 ID 而不是目录名匹配，用户自己装的同名目录不会被误删。
4. **安装时新建的路径**：来自安装记录，例如游戏本身没有的 `steam_appid.txt`。**目录只在其为空时删除**，因此即使用户后来往里放了东西也不会被带走。

另外可选（界面上的复选框，默认关闭）：

5. **`Mods` 目录内的全部内容**：这是不可恢复的操作，界面会显示明确警告；`Mods` 目录本身保留。

游戏自带的文件永远不会被删除：`steam_appid.txt` 只有在安装时不存在（即由本实现创建）的情况下才会被清理。

## 未采用：隐藏 SMAPI 的终端窗口（TODO）

官方启动器脚本（`unix-launcher.sh` → 游戏目录的 `StardewValley`）会打开一个终端窗口显示 SMAPI 的控制台输出，这与 SMAPI 自己写的日志文件是**两套输出**。脚本本身支持 `SMAPI_NO_TERMINAL=true`，走 `--no-terminal` 分支。

曾实现过一版：安装时在脚本 shebang 之后注入 `export SMAPI_NO_TERMINAL="${SMAPI_NO_TERMINAL:-true}"`，并在面板上加开关即时改写脚本。**用户反馈在实际游戏启动时未生效，已整体回滚** —— 代码、命令、状态字段与文案都不再包含这部分。

重新尝试前应先确认现象，再决定做法：

1. 拨开关后读游戏目录的 `StardewValley`，确认注入行是否真的写进去了；
2. 确认窗口是谁开的 —— 本实现只控制启动器脚本，如果游戏由其他方式启动（或窗口其实是 SMAPI 自己拉起的控制台），脚本改写自然不会生效；
3. 若脚本路径确实不通，可考虑「由管理器启动游戏并把输出接到界面」的方案（需求池里的 GAME-02）。

## 游戏日志

SMAPI 每次运行游戏都会重写一份日志，位置在游戏**用户目录**而不是游戏目录：

| 平台 | 路径 |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/StardewValley/ErrorLogs/SMAPI-latest.txt`（通常 `~/.config/…`） |
| Windows | `%APPDATA%\StardewValley\ErrorLogs\SMAPI-latest.txt` |

- 面板上的「查看游戏日志」按需读取，显示路径、更新时间、大小与内容。
- 日志在游戏运行中会持续增长，因此只读取**最后 512 KiB**，界面会提示内容被截断。
- 日志不存在不是错误：说明还没有通过 SMAPI 启动过游戏，界面给出对应提示。
- 只读展示，不做解析或高亮；Mod 加载失败等信息需要读者自行查看。

## 进度与安装日志

安装/卸载运行期间，后端通过 Tauri 事件 `smapi://progress` 上报每个步骤的开始、完成或失败，以及该步骤的耗时：

```jsonc
{
  "step": "download",      // release | download | verify | extract | install | uninstall
  "state": "running",      // running | done | failed
  "detail": null,          // 稳定错误码，或 "cached"；从不放面向用户的文案
  "downloaded": 14680064,  // 仅 download 有效
  "total": 41889142,
  "elapsedMs": 0
}
```

- 下载进度按 120 ms 节流，最后一次（`downloaded == total`）总是发送。
- 事件失败被忽略：窗口关闭或前端未订阅都不会影响安装本身。
- 界面把 `step` 翻译成步骤名、把 `detail` 中的错误码交给 `errors.*` 文案；步骤按首次出现顺序排列，完成后保留在面板上，便于复制诊断。
- 这是界面日志，不写入日志文件；需求池里的持久化日志仍未实现。

## 与官方安装器的差异

复刻不是逐行等价，以下差异是有意为之：

| 项 | 官方安装器 | 本实现 | 原因 |
| --- | --- | --- | --- |
| 遗留文件清理 | 删除 `0Harmony.dll`、`Newtonsoft.Json.dll`、`steam_appid.txt` 等旧版残留 | 只删除本实现写入的文件，加上安装记录中标明「由本实现创建」的路径 | 这些名字与**游戏自带文件重名**（实测 Steam Linux 版自带 `steam_appid.txt`），照抄会破坏可运行的安装 |
| 失败处理 | 先清理后写入，中断即停在中间状态 | 安装前备份受影响路径，失败即回滚 | 需求要求失败可恢复 |
| 用户设置 | 安装前把 `config.user.json` 复制进负载目录 | 安装后写回原内容 | 等价行为，不污染解包目录 |
| 启动器与启动项 | Linux/macOS 替换启动器；Windows 提示用户手动配置 | 同左，且不写入 Steam 配置 | 与 Vortex 的做法一致：由应用启动 `StardewModdingAPI.exe` 更可靠 |
| 依赖 | 安装器进程 | 无（纯 Rust 文件操作） | 目标机器无需 .NET，且可在 CI 中用临时目录完整测试 |
| 可观察性 | 交互式控制台输出 | 结构化进度事件 + 界面步骤日志 | 官方安装器无法被静默调用，其输出也难以解析成错误码 |
| 卸载 | 删除清单内的文件（含游戏自带名字） | 分层清理 + 可选的 Mods 清空 | 既能清干净，又不会误删游戏文件与用户 Mod |

## 检测与版本

- 存在性检测按游戏运行时判断 `StardewModdingAPI.exe`（Windows）或 `StardewModdingAPI`（Linux）。
- 版本只在**由本应用安装**时已知，来源是应用数据目录中的安装记录 `smapi/installed.json`。
- 手动安装、或用官方安装器升级过的 SMAPI 会显示为「版本未知」。本实现不解析程序集版本，也不据此声称兼容性。
- 记录格式当前为版本 2（新增 `created` 路径列表）；版本 1 的旧记录仍可读取，只是卸载时少清理一部分路径。
- 关闭再打开应用不会丢失记录；记录损坏时退化为「版本未知」，不会阻止安装。

## 范围与已知限制

- **macOS 未支持**：命令直接返回 `smapiUnsupportedPlatform`。原生 macOS 安装需要不同负载与启动器处理，未在本次范围内。
- **Windows 未实测**：Windows 分支（负载选择、不替换启动器、`StardewModdingAPI.exe`、`%APPDATA%` 日志路径）只有自动测试覆盖，本机没有 Windows 环境可实测。
- 负载按**游戏的运行时**选择（Windows 游戏文件取 `windows` 负载），因此 Linux 上通过 Wine/Proton 运行 Windows 版游戏也能装到正确的负载。
- 下载缓存位于应用缓存目录，校验通过的文件会被复用；缓存不是安装状态来源，删掉只会重新下载。
- 进度按网络读取节流上报，没有做断点续传：中断的下载会从头开始。
- 步骤日志只保留在本次会话的内存中，重启应用后不再显示。
- 卸载只清理本实现知道的东西；用官方安装器装过的 SMAPI 残留（例如老版本文件）不会被本实现识别。
- 回滚是把安装前移动走的原文件移回；如果回滚本身也失败（例如权限突然变化），返回的错误详情会同时带上原始失败与回滚失败，不会掩盖前者。
- 应用在安装过程中被强杀时，游戏目录里可能留下 `.lingrid-install-*` 临时目录：里面是安装前的原文件，可手工移回原位。正常成功或失败的安装都会删掉它。

## 接口

| 命令 | 输入 | 返回 |
| --- | --- | --- |
| `smapi_status` | `{ path }` | `{ installed, version, executable }` |
| `fetch_smapi_release` | 无 | `{ version, assetName, assetUrl, sha256, size }` |
| `smapi_log` | 无 | `{ path, exists, modifiedMs, size, content, truncated }` |
| `install_smapi` | `{ path }` | `{ version, executable, launcherReplaced }` |
| `uninstall_smapi` | `{ path, removeMods }` | `{ removed, modsRemoved, launcherRestored }` |

事件：`smapi://progress`，载荷见上。

错误沿用 `{ code, detail }`，新增代码：`smapiUnsupportedPlatform`、`smapiUnavailable`、`smapiReleaseUnavailable`、`smapiReleaseInvalid`、`smapiDownloadFailed`、`smapiChecksumMismatch`、`smapiArchiveUnreadable`、`smapiArchiveInvalid`、`smapiInstallFailed`、`smapiNotInstalled`、`smapiGameFilesMissing`、`smapiRecordFailed`、`smapiLogUnavailable`、`smapiLogUnreadable`。

## 验证

```sh
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked
```

自动测试覆盖：Linux/Windows 安装布局、启动器替换与恢复、捆绑 Mod 白名单、更新时保留原版启动器备份与用户设置、卸载（含捆绑 Mod 清理、安装新建文件的清理、游戏自带文件的保留、清空 Mods）、失败回滚、路径穿越与符号链接拒绝、记录往返/损坏退化/旧格式兼容、SHA-256 校验（含篡改检测）、日志读取与截断、IPC 契约、进度事件合并、卸载确认流程。

两个可选实测（默认跳过，不会修改本机游戏）：

```sh
# 真实发行包 + 本机游戏的临时副本，端到端安装与卸载
SMAPI_INSTALLER=/path/to/SMAPI-4.5.2-installer.zip \
STARDEW_GAME="$HOME/.local/share/Steam/steamapps/common/Stardew Valley" \
cargo test --manifest-path src-tauri/Cargo.toml --lib installs_the_real_release -- --ignored --nocapture

# 真实网络：解析最新版本、下载（并检查字节进度）、校验哈希与篡改拒绝
cargo test --manifest-path src-tauri/Cargo.toml --lib downloads_and_verifies_the_latest_release -- --ignored --nocapture
```

2026-09-13 的验证记录：

- `npm test` 28 项通过，`npm run build` 通过（含三语言 134 键校验与 `vue-tsc`）。
- `cargo test --lib` 31 项通过、3 项跳过；`cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --check` 无输出。
- 用真实发行包 `SMAPI-4.5.2-installer.zip`（sha256 `dd01ddca…acc4`，与 GitHub API 的 `digest` 一致）对**本机 Steam Linux 版游戏的临时副本**完成安装与卸载；原版启动器按字节还原。
- **真实使用验证**：用户用本应用把 4.5.2 装进本机游戏目录后启动游戏，SMAPI 日志显示 `SMAPI 4.5.2 with Stardew Valley 1.6.15` 并成功加载了两个捆绑 Mod —— 这是整条链路（下载、校验、解包、落盘、启动器替换）的端到端证据。
- 联网实测解析到 4.5.2、下载过程中持续上报字节进度、校验通过、截断文件被校验拒绝。
- 真实数据发现过一个自动测试漏掉的问题：SMAPI 的 `manifest.json` 使用 `UniqueId` 字段名，最初按 `UniqueID` 匹配会导致捆绑 Mod 全部被过滤；现已按大小写不敏感匹配。

**未验证**：Windows 分支的实际行为、真实 Windows 游戏目录与 `%APPDATA%` 日志路径、macOS 支持、官方安装器与本实现产物的逐字节等价性，以及界面中进度、日志与卸载确认的真实观感（本机无桌面会话）。卸载路径尚未在真实游戏目录上执行过，只有临时副本与自动测试覆盖。

## 参考

- [SMAPI 发行版（GitHub Releases）](https://github.com/Pathoschild/SMAPI/releases)
- [SMAPI 官方安装说明](https://stardewvalleywiki.com/Modding:Player_Guide)
- [SMAPI 源码：InteractiveInstaller.cs](https://github.com/Pathoschild/SMAPI/blob/develop/src/SMAPI.Installer/InteractiveInstaller.cs)
- [SMAPI 许可证（LGPL-3.0）](https://github.com/Pathoschild/SMAPI/blob/develop/LICENSE.txt)

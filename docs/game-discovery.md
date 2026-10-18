# 游戏目录识别：实现与验证

本次根据用户最新指令实现 GAME-01：Windows/Linux 游戏目录自动识别、手动选择与校验。原需求文档中的“基础版暂不做自动发现”不再限制本次工作。没有实现 Mod 扫描、安装、游戏启动或内置 SMAPI。

## 结构与职责

```text
src/
  features/game/
    types.ts                  IPC 数据类型及运行时校验
    api.ts                    游戏命令调用
    store.ts                  扫描、选择、恢复和忙碌状态
    GameDirectoryPage.vue     游戏目录页面
  shared/ipc.ts               集中调用及错误归一化
  shared/preferences.ts       界面偏好读写及失败提示
  i18n/locales/*.json         单文件语言元信息和文案
src-tauri/src/
  error.rs                    可本地化的错误代码与诊断信息
  game/
    commands.rs               Tauri 接口和阻塞任务调度
    model.rs                  结果、来源、运行平台、警告
    platform/{linux,windows}.rs  系统环境、注册表、候选路径
    discovery.rs              Steam 库展开、清单解析、校验、去重
    validation.rs             实际游戏目录校验
    vdf.rs                    有大小/深度限制的 Steam 文本 VDF 解析
    tests.rs                  临时目录测试和本机只读检查
```

页面不读文件、不拼接平台路径、不调用系统命令。平台适配只提供候选位置；发现服务负责读取和验证。全部文件扫描及原生目录选择在 `spawn_blocking` 中运行。新增平台时增加适配模块并在 `platform/mod.rs` 接入；目前其他平台的自动发现返回 `platformUnsupported`。

## 自动发现范围

| 系统 | 位置 |
| --- | --- |
| Windows | HKCU/HKLM 的 Steam 安装路径、Steam App 413150 卸载记录、GOG Games 注册表记录，读取 32/64 位注册表视图 |
| Windows | Program Files 系列环境变量下的 Steam、GOG Galaxy；系统盘常见 GOG、Xbox 路径；LocalAppData 下的 Steam |
| Linux | `~/.steam/steam`、`~/.steam/root`、`~/.steam/debian-installation`、`~/.local/share/Steam`、有效的绝对 `XDG_DATA_HOME` 下的 Steam |
| Linux | Flatpak Steam、Snap Steam 常见路径 |
| Linux | `~/GOG Games`、`~/GOGGames` 下的常见游戏目录，以及 Heroic 常见目录 |

Steam 根目录自身也作为游戏库。读取 `steamapps/libraryfolders.vdf` 及旧位置 `config/libraryfolders.vdf`，兼容新对象格式和旧字符串格式。每个游戏库读取 `steamapps/appmanifest_413150.acf`，校验 App ID 和 `installdir`，同时检查默认的 `steamapps/common/Stardew Valley`。

支持自定义 Steam 库、多盘、路径中的空格及中日文。利用 canonicalize 合并符号链接指向的同一安装，保留多个不同安装供用户选择。不递归扫描整块磁盘。自定义 GOG/Heroic 路径、Wine 前缀、未登记的游戏副本、其他盘的 Xbox 自定义目录需手动选择；没有宣称穷尽所有发行渠道。

## 校验与结果

- 要求绝对路径、目录可读、存在 `Content` 文件夹。
- Windows 文件布局：存在 `Stardew Valley.exe`。
- Linux 文件布局：存在 `StardewValley` 启动文件，且有受识别的游戏程序集或旧版二进制。
- 根据文件布局返回 `runtime`，不是根据管理器运行系统推断；因此在 Linux 中发现 Windows 游戏文件时会明确标记。
- 只报告 `StardewModdingAPI` / `StardewModdingAPI.exe` 是否存在，不宣称 SMAPI 已正确安装、可执行或版本兼容。
- 这是目录特征校验，不是完整性校验、可启动性测试或游戏版本兼容性认证。
- 不存在的默认路径正常跳过。无权限、损坏清单、无效目录作为警告返回，其他位置继续扫描。
- VDF 最多读取 1 MiB，嵌套深度限制 32；拒绝 `installdir` 中的绝对路径、父目录及路径分隔符。

接口：

| 命令 | 输入 | 返回 |
| --- | --- | --- |
| `discover_game_installations` | 无 | `{ installations, warnings }` |
| `validate_game_directory` | `{ path }` | `GameInstallation` |
| `pick_game_directory` | 无 | `GameInstallation` 或取消时的 `null` |

错误统一为 `{ code, detail }`。前端翻译 `code`，诊断文字可折叠查看；数据格式不匹配时返回明确错误。目录对话框通过 Rust 的 Tauri dialog 插件调用，没有向前端开放通用文件系统或 shell 权限。

## 选择与语言

界面采用桌面应用外壳：左侧固定导航，右侧页面独立滚动，顶部显示当前位置，底部显示目录状态。当前接入概览、游戏目录、设置三个页面；语言选择位于设置页。窗口默认 1120×760、最小 640×480，窄窗口下侧栏缩为带提示的图标导航。

`src/app/router.ts` 使用 Hash 路由，适配 Tauri 静态资源加载；`src/app/navigation.ts` 定义菜单。新增页面时在对应功能目录创建组件，再注册路由和导航项、补齐三种语言。`App.vue` 负责外壳和启动扫描，页面通过 `KeepAlive` 保留输入状态；切换菜单不会重新扫描。概览与目录页共用 Pinia 中的实际扫描结果。

首次发现唯一副本时自动选中；存在多副本时让用户选择。目录和语言偏好分别保存在 WebView localStorage 的 `lingrid.gamePath.v1`、`lingrid.locale.v1`。下次扫描会重新验证手动保存的目录；旧目录失效时提示用户，不自动换到别的副本。浏览器预览明确提示需使用桌面应用。

这些是此功能的界面偏好，不是原需求文档中完整的 Rust 设置服务；设置文件迁移、损坏文件备份等仍待后续实现。localStorage 访问失败会显示提示，当前操作仍可继续。偏好按桌面应用的 WebView 用户数据持久化，浏览器预览的存储与桌面应用不同。

语言文件通过 `import.meta.glob` 自动发现。初始含简中、英语、日语；新增语言只需复制一份 JSON 并修改元信息、完整翻译。构建前检查元信息、重复语言标识、翻译键及插值参数。运行时支持切换、英语回退、HTML lang/dir 更新。当前文案约定使用普通字符串和命名插值；更复杂的复数/富文本规则应另行扩展验证。

## 验证

```sh
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked
```

`npm test` 使用 Node 自带测试器和 TypeScript 类型剥离，开发/CI 使用 Node 24 或更高版本。Rust 测试使用临时目录，不要求安装游戏，也不会修改用户的游戏文件。

本机实际安装的只读扫描（默认测试跳过此项）：

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib discovers_host_installations -- --ignored --nocapture
```

2026-09-13：Linux 本机只读扫描成功识别已安装的 Steam Linux 版，未产生警告。自动测试覆盖两种游戏文件布局、自定义 Steam 库、新旧 VDF、清单损坏、路径穿越、中文路径、多副本、符号链接、配置恢复及 IPC 格式校验。

已提供 `.github/workflows/check.yml`，在 Ubuntu 24.04 和 Windows runner 上构建并检查。新增 CI 文件不代表 Windows CI 已执行；当前工作目录没有 Git 仓库，需要提交到 GitHub 后才会触发。Windows 注册表读取、Windows/Linux 原生目录对话框和桌面视觉交互仍需在相应桌面环境实测。

## 参考

- [Tauri：从前端调用 Rust](https://v2.tauri.app/develop/calling-rust/)
- [Tauri：原生目录对话框](https://v2.tauri.app/plugin/dialog/)
- [星露谷物语 Modding 指南：游戏目录](https://wiki.stardewvalley.net/Modding:Player_Guide/Getting_Started)

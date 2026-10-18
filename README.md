# Lingrid

星露谷物语 Mod 管理器，使用 Tauri 2、Vue 3、TypeScript 和 Rust。

当前已实现：

- **游戏目录**：Windows/Linux 自动发现、手动选择与目录校验。
- **SMAPI**：检测、安装、更新与卸载（从 GitHub 官方发行包下载、校验 SHA-256、失败回滚，显示分步进度与操作日志，卸载时可一并清空 Mods；并可在界面内查看游戏运行日志）。
- **Mod 管理**：扫描已安装 Mod、启用停用（SMAPI 的点前缀约定）、删除、从 zip 安装。
- **Nexus 集成**：匿名搜索 Nexus Mods（按名称，或直接输入社区分享的 Mod ID）；注册 `nxm://` 协议，网页上点「Mod Manager Download」即可唤起应用下载安装（需要自己的 Nexus API key，免费账号可用），也支持手动粘贴链接。
- **界面**：简体中文、英语、日语，亮色/暗色/跟随系统与四组主题色。

尚未实现：游戏启动、Mod 更新检测、`.7z`/`.rar` 包。

## 开发

安装 Node.js 24+、Rust stable 及 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/)，然后运行：

```sh
npm ci
npm run tauri dev
```

`npm run dev` 仅打开前端开发服务；本机目录访问需要 Tauri 桌面应用。

### 打包 AppImage

```sh
NO_STRIP=1 npm run tauri build -- --bundles appimage
```

产物在 `src-tauri/target/release/bundle/appimage/`。Arch 等使用新版 binutils 的发行版需要 `NO_STRIP=1`：Tauri 调用的 linuxdeploy 自带旧版 `strip`，不认识 `.relr.dyn` 段，会在打包阶段中止（应用本身已经编译成功）。

### 已知问题：NVIDIA 下窗口灰屏

WebKitGTK 的 DMA-BUF 渲染器在 NVIDIA 驱动上经常分配缓冲区失败，页面已经加载但窗口只剩一块灰色。应用检测到 `/proc/driver/nvidia/version` 时会自动设置 `WEBKIT_DISABLE_DMABUF_RENDERER=1`；如果你自己设置过这个变量，应用不会覆盖。仍然异常时可以再叠加：

```sh
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./lingrid_0.1.1_amd64.AppImage
```

## 检查

```sh
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked
```

## 文档

- [产品需求](docs/需求文档.md)
- [目录识别：架构、覆盖范围、接口、扩展方式和验证记录](docs/game-discovery.md)
- [SMAPI 安装：流程、进度上报、与官方安装器的差异、限制和验证记录](docs/smapi.md)
- [Mod 管理与 Nexus 集成：扫描规则、安装流程、下载授权和验证记录](docs/mods.md)
- [主题系统：调色板变量、跟随系统行为、新增主题色的步骤](docs/theme.md)

Linux 本机发现流程与 SMAPI 安装/卸载已验证（含真实发行包对游戏副本的端到端测试）；Windows 原生行为、Windows 分支的 SMAPI 安装仍需 Windows 环境验证，macOS 支持尚未实现。Nexus 的匿名搜索已实测，下载全流程需要真实账号签发的链接才能验证。主题与界面观感需要桌面环境人工确认。GitHub Actions 已配置 Windows/Linux 构建检查，运行结果以实际 CI 为准。

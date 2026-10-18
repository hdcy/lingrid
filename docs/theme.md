# 主题系统：实现与扩展

界面支持亮色 / 暗色 / 跟随系统三种外观，以及四组预设主题色。所有颜色集中在一处，组件样式只引用变量。

## 结构

```text
src/styles.css          调色板定义（模式 × 主题色的变量）与基础样式
src/shared/theme.ts     主题状态、跟随系统、持久化与校验
src/features/settings/SettingsPage.vue  设置页的外观与主题色选择
index.html              渲染前应用已保存的主题，避免暗色下闪白
```

组件不写死颜色：`App.vue`、`SmapiPanel.vue` 等只使用 `var(--…)`。新增界面时同样只引用变量。

## 变量

语义变量按用途命名，与具体色值无关：

| 变量 | 用途 |
| --- | --- |
| `--bg-app` `--bg-panel` `--bg-sidebar` `--bg-header` `--bg-statusbar` `--bg-input` `--bg-subtle` `--bg-hover` | 各层背景 |
| `--text` `--text-muted` | 正文与次要文字 |
| `--border` `--border-strong` | 分隔线与控件边框 |
| `--accent` `--accent-hover` `--accent-contrast` `--accent-text` `--accent-soft` `--accent-soft-border` `--accent-tint` | 主题色及其派生色 |
| `--warning` `--danger-*` `--success-*` `--notice-*` `--focus` | 状态与焦点 |

模式决定中性色，主题色决定强调色。因此 `:root[data-theme='dark'][data-accent='ocean']` 这样的组合各有一组值：**亮色下的浅色调在深色背景上不可读**，暗色模式必须给更亮的 `--accent` 和更深的 `--accent-soft`。

## 行为

- 状态写在 `<html>` 的 `data-theme`（`light` / `dark`）与 `data-accent` 上，CSS 完全由这两个属性驱动。
- 模式为「跟随系统」时监听 `prefers-color-scheme` 变化并即时切换；系统变化不需要重启。
- 选择保存在 WebView 的 localStorage（`lingrid.themeMode.v1`、`lingrid.themeAccent.v1`），与语言、游戏目录偏好一致。
- 读取到无效值（旧版本、手工修改）时回退到「跟随系统」与默认主题色，不会出现无样式状态。这条有单元测试覆盖。
- `index.html` 在应用脚本之前应用一次主题；`src/shared/theme.ts` 启动后接管同一份状态。

## 新增一组主题色

1. 在 `themeAccents`（`src/shared/theme.ts`）中加入标识符，并补 `docs` 中的说明。
2. 在 `src/styles.css` 中为该标识符补两组变量：亮色一组、`:root[data-theme='dark'][data-accent='…']` 一组。每组需要 `--accent`、`--accent-hover`、`--accent-contrast`、`--accent-text`、`--accent-soft`、`--accent-soft-border`、`--accent-tint`。
3. 在设置页的色板（`SettingsPage.vue` 的 `.swatch-*`）里补一个色块，并补齐三种语言的名称。
4. 检查对比度：按钮文字对 `--accent`、正文对 `--accent-soft` 都要可读；暗色下不要把亮色的浅色调直接搬过来。

## 验证

```sh
npm test
npm run build
```

自动测试覆盖无效持久化值的回退（`tests/theme.test.mjs`）；`npm run build` 会做三语言键一致性与 `vue-tsc` 类型检查。

2026-09-13：21 项前端测试通过，构建通过。设置页在三种模式下均使用变量，无遗留硬编码颜色（`grep -n '#[0-9a-f]' src --include='*.vue' --include='*.css'` 只应命中 `styles.css` 的调色板定义与设置页的色板样本）。

**未验证**：本机没有桌面会话，实际窗口中的视觉对比度、系统主题切换的即时性、以及各主题色在真实屏幕上的观感需要人工确认。

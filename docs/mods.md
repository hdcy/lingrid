# Mod 管理与 Nexus 集成：实现与验证

本次实现需求池中的 MOD-01～MOD-04：扫描并展示已安装的 Mod、启用停用、删除、从本地压缩包安装，以及浏览 Nexus Mods 并把 Mod 装进游戏目录。

## 结构与职责

```text
src-tauri/src/mods/
  model.rs      InstalledMod、安装结果
  scan.rs       扫描 Mods 目录、解析 manifest.json
  manage.rs     启用/停用（重命名）与删除
  install.rs    从 zip 安装：解压、定位 manifest、落盘
  commands.rs   Tauri 命令
src-tauri/src/nexus/
  model.rs      搜索结果类型
  graphql.rs    匿名搜索（v2 GraphQL）
  nxm.rs        nxm:// 解析、换取下载地址、下载
  credentials.rs  API key 的存储（0600 权限）
  commands.rs   Tauri 命令
src/features/mods/    Mod 管理页面与状态
src/features/nexus/   Nexus 浏览页面、nxm 安装入口，API key 设置
```

页面不读文件、不拼平台路径。所有命令都先用游戏目录校验解析出 `Mods`，因此任意路径都无法用来改名或删除别处的文件。

## Mod 扫描与启用停用

- 扫描 `游戏目录/Mods` 的每个子目录，读取其中的 `manifest.json`（找不到时往下一层找，兼容把内容包在一层目录里的打包方式）。
- 字段按大小写不敏感读取（SMAPI 各版本拼写不一致）；`UpdateKeys` 里的 `Nexus:<id>` 会解析出来，用于「在 Nexus 打开」。
- **停用 = 文件夹名加前导点**，这是 SMAPI 本身的约定（`folder.Name.StartsWith(".")` → `ModType.Ignored`）。启用即去掉该点。
- `Mods/.cache`、`__MACOSX`、`.DS_Store` 以及没有 manifest 的点开头目录会被忽略；没有 manifest 的普通目录仍会列出，标记为「无清单文件」（XNB Mod 和内容包就是这种）。
- 改名时如果目标名已被占用，操作会被拒绝而不是覆盖；删除接受带点或不带点的名字，并拒绝任何包含路径分隔符或 `..` 的输入。

## 从压缩包安装

- 只支持 zip（Nexus 上 Stardew Mod 的主流格式）；解压到临时目录后，按 `manifest.json` 定位 Mod 根，一层或两层目录都能识别。
- **一个包可以包含多个 Mod**，会全部安装。
- 文件夹名取包内的目录名；如果 Mod 直接位于包根，则用压缩包文件名。名字会清洗非法字符，并去掉前导点——否则装进去就是禁用状态。
- 目标文件夹已存在时报错（`modFolderTaken`）而不是覆盖，避免悄悄丢掉用户对旧版本所做的修改。
- 安装前先校验每个 `manifest.json`：`Name`、`Author`、`Version`、`UniqueID` 缺一不可（字段名大小写不敏感，`Version` 可以是字符串或对象），否则报 `modManifestInvalid`。SMAPI 会拒绝加载清单损坏的 Mod，提前拦住比装完再报错更好。
- 同一个 `UniqueID` 已在 `Mods` 中存在（不论启用与否），或同一个包里出现两次时，报 `modDuplicate` —— SMAPI 遇到重复 ID 会直接跳过重复的那个，用户很难看出原因。目录名相同仍优先报 `modFolderTaken`，因为「删掉旧版本」是更直接的解法。
- 解压会拒绝绝对路径、`..` 穿越与符号链接条目。

## Nexus 集成

### 搜索（无需登录）

`https://api.nexusmods.com/v2/graphql`，实测**匿名可查**，无需 API key：

```graphql
query Search($game: String!, $name: String!, $count: Int!) {
  mods(filter: { gameDomainName: [{ value: $game }], name: [{ value: $name, op: WILDCARD }] }, count: $count) {
    nodes { modId name summary version downloads author }
  }
}
```

游戏域为 `stardewvalley`。空查询会走另一条只按游戏过滤的查询，用于浏览最新 Mod。按 Nexus API 使用条款，请求带 `Application-Name`、`Application-Version` 与可识别应用的 User-Agent。

### 按 Mod ID 查询

社区分享 Mod 时通常给的是 ID（例如「装一下 5815」）而不是链接，所以搜索框里**输入纯数字就按 ID 查**，其余输入按名称搜：

```graphql
query ById($game: String!, $id: String!, $count: Int!) {
  mods(filter: { gameId: [{ value: $game }], modId: [{ value: $id }] }, count: $count) {
    nodes { modId name summary version downloads author }
  }
}
```

两个容易踩的点，都是实测确认的：

- 按 `modId` 过滤时端点**要求同时给 `gameId`**，用 `gameDomainName` 会报 `gameId is required when filtering by modId`；星露谷的数值 ID 是 **1303**（与 MO2 的 `GameNexusId` 一致）。
- `modId` / `gameId` 的 `value` 必须是**字符串**（`BaseFilterValue.value` 是 `String!`），传数字会被拒。

实测：查 `5815` 返回 `Fishing Assistant 3`（作者 I3lanketGhost，版本 3.4.1）。

### 下载（需要 API key，免费账号可用）

Nexus 的限制是服务端强制的：**非 Premium 账号不能直接调用下载接口**，必须带上网站签发的 `key` 与 `expires`。所以下载链路是：

1. 用户在 Nexus 网页点「Mod Manager Download」得到 `nxm://stardewvalley/mods/<id>/files/<id>?key=…&expires=…`；
2. 把该链接粘贴进「从链接安装」（面板里给的就是这个入口）；
3. 应用解析链接（游戏域必须是 `stardewvalley`，否则拒绝），用保存的 API key 调 `…/files/<id>/download_link.json` 换取临时下载地址；
4. 下载到临时文件后用与本地安装相同的流程装进 `Mods`。

下载有三道约束：只接受 `https` 且最终主机属于 `*.nexusmods.com` / `*.nexus-cdn.com`（防止 `download_link.json` 被伪造或跳转到别处）、总量不超过 4 GiB、写盘前校验 zip 文件头。任一不满足都会中止，临时文件被删除。

API key 在设置页填入，保存在应用数据目录的 `nexus/credentials.json`（Unix 下 0600 权限），**不会回传给界面**——界面只询问「是否已保存」。

## 与同类实现的差异

Mod Organizer 2 的源码显示它拦截 `nxm://` 协议并把链接交给自己的下载器（`NXMAccessManager::createRequest`、`NXMUrl` 解析出 `key`/`expires`/`user_id`），免费账号走的正是这条带 key 的路径。本实现复用了同一套下载授权逻辑，并同样注册了系统协议：

| 项 | MO2 | 本实现 |
| --- | --- | --- |
| 下载授权 | nxm 链接的 key/expires | 同左 |
| 接收链接 | 注册 `nxm://` 系统协议，网页点击即唤起 | 同左，外加手动粘贴入口 |
| 认证 | OAuth SSO 为主，API key 兜底 | 仅 API key |

### 注册 nxm:// 协议

网页上的「Mod Manager Download」把 `nxm://…` 交给系统，系统再转给认领了该协议的应用。认领由三部分组成：

- `tauri-plugin-deep-link` 声明并注册 `nxm` scheme（`tauri.conf.json` 的 `plugins.deep-link.desktop.schemes`）；
- `tauri-plugin-single-instance`（启用 `deep-link` feature）**必须最先注册**——浏览器启动的是**第二个进程**，它把参数交给已在运行的实例而不是再开一个窗口；回调里顺带把主窗口带到前台；
- Rust 侧通过 `on_open_url`（运行中收到的链接）与 `get_current`（应用就是被链接启动的情况）把 URL 转发成 `nxm://received` 事件，前端收下后装进当前选中的游戏目录。

启动时还会调用 `register_all()` **在运行时注册**一次，所以从源码运行的二进制、开发模式或 AppImage 也能认领协议，不必先做成安装包。注册失败只打印一行日志，不影响应用启动。

游戏未选择时链接不会被丢掉：界面报 `nexusNeedsGame`，提示先选目录再点一次。

**已知限制**：

- 运行时注册的桌面条目指向**当前可执行文件的绝对路径**。AppImage 被移动到别处后旧条目失效，重新启动一次该 AppImage 就会重新注册（`register_all()` 每次启动都执行）。
- 开发模式注册的条目指向 `target/debug/lingrid`，需要 dev server 同时运行才有意义。
- macOS 不能运行时注册，必须在打包时声明。

### 链接的两种形式

网页上能看到两种链接，用途不同：

| 形式 | 例子 | 行为 |
| --- | --- | --- |
| 文件链接 | `nxm://stardewvalley/mods/5815/files/123456?key=…&expires=…` | 用它换取下载地址并安装 |
| Mod 链接 | `nxm://stardewvalley/mods/5815`、`https://www.nexusmods.com/stardewvalley/mods/5815` | 只指向 Mod，**没有文件也没有下载授权**；界面会打开该 Mod 页面，让用户去 Files 标签选文件 |

只带 mod id 的链接返回 `nxmNeedsFile`，而不是笼统的「无效链接」——它是合法链接，只是不够具体。

协议没生效（例如开发模式）时，还有两条路可走：把链接**粘贴**进「从链接安装」，或者用文件的 Manual download 下载 zip 再用「从文件安装」——后者连 API key 都不需要。

## 范围与已知限制

- **只支持 zip**：`.7z` / `.rar` 会报「压缩包内容异常」。Nexus 上这类包少见但存在。
- **搜索结果是按相关度返回的前 20 条**，没有分页、排序、分类或按作者筛选。
- **不读取 Mod 的远程详情页**（描述、图片、文件列表需要额外的 API 调用与 key）。
- **不做 Mod 更新检测**：只有 SMAPI 自己会提示更新，管理器不比对版本。
- 删除操作**不会进回收站**，直接 `remove_dir_all`。
- Nexus 的下载次数配额（免费/Premium 的每日上限）未在界面中提示，超限时表现为下载被拒。
- 修改 `Mods` 目录前不会检查游戏是否在运行；游戏运行中改名或删除可能导致 SMAPI 报错（界面错误文案里提示先关闭游戏）。

## 接口

| 命令 | 输入 | 返回 |
| --- | --- | --- |
| `list_mods` | `{ path }` | `InstalledMod[]` |
| `set_mod_enabled` | `{ path, folder, enabled }` | 无 |
| `remove_mod` | `{ path, folder }` | 无 |
| `pick_mod_archive` | 无 | 路径或取消时的 `null` |
| `install_mod` | `{ path, archive }` | `{ folders }` |
| `search_nexus_mods` | `{ query }` | `NexusModSummary[]` |
| `nexus_api_key_status` | 无 | `boolean` |
| `set_nexus_api_key` | `{ key }` | 无 |
| `install_mod_from_nxm` | `{ path, url }` | `{ folders }` |

新增错误码：`modsUnreadable`、`modArchiveUnreadable`、`modArchiveInvalid`、`modInstallFailed`、`modFolderTaken`、`modFolderInvalid`、`modNotFound`、`modRenameFailed`、`modRemoveFailed`、`nexusUnavailable`、`nexusQueryFailed`、`nexusKeyMissing`、`nexusKeyNotSaved`、`nexusDownloadDenied`、`nxmInvalid`、`nxmUnsupportedGame`。

## 验证

```sh
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --locked
```

自动测试覆盖：manifest 解析（含 `Nexus:` 更新键）、禁用目录的识别与还原、包装目录与无清单目录、忽略 `Mods/.cache`、启停重命名与冲突拒绝、删除的路径穿越拒绝、zip 安装（包装结构/根目录/多 Mod/冲突/穿越/前导点），以及前端的状态流转与 IPC 契约。

2026-09-13 的验证记录：

- `npm test` 34 项通过，`npm run build` 通过（含三语言 201 键校验与 `vue-tsc`）。
- `cargo test --lib` 46 项通过、3 项跳过；`cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --check` 无输出。
- **Nexus 匿名搜索已实测**：对 `api.nexusmods.com/v2/graphql` 发 POST，返回了真实结果（`mods(filter: { gameDomainName: [{ value: "stardewvalley" }], name: [{ value: "Smapi", op: WILDCARD }] })` → `modId`/`name`/`summary`/`version`/`downloads`/`author`）。
- **按 ID 查询已实测**：`mods(filter: { gameId: [{ value: "1303" }], modId: [{ value: "5815" }] })` 返回 `Fishing Assistant 3`。
- Nexus 的下载端点、API key 认证与 `nxm://` 换取流程**未在本机实测**：需要一个已登录账号签发的 nxm 链接，且要在游戏目录里真实安装一次。

**未验证**：真实 Nexus 下载与安装全流程；`nxm://` 协议注册只做到「插件接入 + 编译通过」，**没有在打包安装后实测网页点击能否唤起应用**；`.7z`/`.rar` 包；界面观感（本机无桌面会话）。

## 参考

- [Nexus Mods API 文档](https://app.swaggerhub.com/apis-docs/NexusMods/nexus-mods_public_api_params_in_form_data/1.0)
- [Nexus API 使用条款](https://help.nexusmods.com/article/114-api-terms-of-service)
- [Mod Organizer 2 的 nxm 处理](https://github.com/ModOrganizer2/modorganizer/blob/master/src/nxmaccessmanager.cpp)
- [SMAPI 的 Mod 扫描规则](https://github.com/Pathoschild/SMAPI/blob/develop/src/SMAPI.Toolkit/Framework/ModScanning/ModScanner.cs)

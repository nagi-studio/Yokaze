# Yokaze（夜風）

高颜值的第三方网易云播放器，原名 YesPlayMusic，是从 [qier222/YesPlayMusic](https://github.com/qier222/YesPlayMusic)
分出来独立维护的版本，v0.13.0 起改名 Yokaze。Apple Silicon macOS 是正式支持平台；Windows x64 和
Ubuntu x64 由 CI 提供 Tauri 实验构建。

## 命令

日常脚本看 `package.json`：`bun run dev:tauri` 开发，`build:tauri[:windows|:linux]` 出包，
`build:renderer` 只构建渲染进程。不在里面的：

- ypm TUI：`cargo build --release --manifest-path src-tauri/Cargo.toml -p yesplaymusic-tui`
- Tauri 产物在 `src-tauri/target/<target-triple>/release/bundle/`；macOS 正式发布用
  `bun run package:tauri:dmg` 收集到 `dist_tauri/`

## 提交前的验证

`.githooks/pre-commit` 会跑 `bun test`、`bun run typecheck` 和
`bun run build:tauri:renderer`。
`bun install` 时的 `prepare` 会把 `core.hooksPath` 指过去，新 clone 也自动生效。

三步缺一不可：测试不 import `.vue`，所以"import 了一个不存在的模块"只有类型检查或渲染构建能发现——
2026-08-04 就是这么把临时探针的残留 import 提交进去的，HEAD 里 import 了一个仓库里
根本不存在的文件。

CI（`.github/workflows/build.yaml`）只验证每次 push 的**最后一个 commit**，一次推 21 个
中间那 20 个不会被碰，所以这道关必须在本地。

改了 Rust 还要自己跑 `cargo test --workspace`、`cargo clippy --workspace --all-targets`
和 `cargo fmt --all -- --check`（都在 `src-tauri/` 下）。**fmt 是 CI 门禁但不在 pre-commit
里**，忘了跑就会在 CI 上红。

改了 `#[cfg(windows)]` 的代码（`taskbar_docking/`、`win32.rs`、`qq_music_login.rs` 等），
在 Mac 上加跑一遍 `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`。
Windows CI 也会 test + clippy 主程序，但一轮要 40 多分钟，本地先拦住能省一轮；
建窗口的原生测试（如 `taskbar_docking` 的重绘、全屏用例）在 Mac 上跑不了，只能靠 CI。

## 发版

版本号要同时改六处：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、
`src-tauri/core/Cargo.toml`、`src-tauri/sidecar/Cargo.toml`、`src-tauri/tui/Cargo.toml`（Cargo.lock
里的四个 workspace package 跟着更新）。
`bun run verify:tauri:version` 会校验所有位置与 tag 一致，CI 里也会跑。

推 `v*` tag 触发 `.github/workflows/build.yaml`：三平台构建（macOS 含 Developer ID 签名与公证）→
建**草稿** release。正式版草稿用 `gh release edit vX.Y.Z --draft=false --latest` 发布；
canary 等预发布版本必须用
`gh release edit vX.Y.Z-canary.N --draft=false --prerelease --latest=false`，不能设为 latest。
canary 发布后的 `release.published` 会触发 `.github/workflows/publish-canary-updater-feed.yaml`：
它只在最终 artifact 与 `TAURI_UPDATER_PUBKEY` 验签通过后推进独立 canary feed；不要手工改
`updater-feed` 分支，也不要让草稿提前进入 feed。stable 继续使用 GitHub latest，不会收到 canary。

macOS 正式发布走 Developer ID 签名 + Apple 公证，**没有开关**：tag 构建一律把
`APPLE_CERTIFICATE` 导进临时钥匙串出签名 DMG，`codesign --verify`、`spctl --assess`
和 `xcrun stapler validate` 三项都是硬门禁，任一失败就不出包。签名身份是
`Developer ID Application: Zexi Zhang (PJM828YBFJ)`。v0.9.1 及更早的包仍是 adhoc。

曾经有过一个 `APPLE_SIGNING_ENABLED` 变量，v0.9.3 删掉了：它的 `!= 'true'` 分支会在
变量缺失或拼错时静默退回 adhoc 出包，而下游没有任何一步检查这件事——配置漂移会安静地
发出一个没签名的正式版。**不要再引入这种"签名可选"的旁路。**

DMG 要单独送公证并 staple。Tauri 只公证 `.app`，而 quarantine 是打在 DMG 上的、
macOS 第一次打开时评估的也是磁盘映像本身，所以少了这步的包在用户那里照样被拦。

七个 secret 缺一个就卡在 "Verify Apple release secrets"：`APPLE_CERTIFICATE`（p12 的
base64，**只含 Developer ID 这一张**，login 钥匙串里另外两张身份不要一起导）、
`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD`、
`APPLE_TEAM_ID`、`KEYCHAIN_PASSWORD`。`APPLE_PASSWORD` 是 appleid.apple.com 生成的
**App 专用密码**，不是账号登录密码；换密码前用
`xcrun notarytool store-credentials <名字> --apple-id <邮箱> --team-id PJM828YBFJ`
本地验一次，它会当场向 Apple 校验，比打 tag 等 CI 失败快得多。

**证书 2027-02-01 到期。** 签名带安全时间戳，到期前签出的包之后依然有效，但之后要发新版本
必须先续证书、重新导出 `APPLE_CERTIFICATE` 和 `APPLE_CERTIFICATE_PASSWORD`。

Tauri updater 的 Minisign 密钥是另一套完整性门禁，和 Developer ID 互不替代，两套都不能关。

`draft-release` 汇总产物时会用 updater 私钥给三个 `ypm-*` 二进制出 `.sig`，
`ypm update` 靠它验签；`generate-updater-manifest.mjs` 按 `ypm-` 前缀跳过这些文件。
草稿 release 无法用 `gh release edit <tag>` 定位（tag 还没绑上），改用
`gh api -X PATCH repos/<owner>/<repo>/releases/<id>`。GitHub 对同一个 release 只发一次
`release.published`，所以发布后才修 tag 会让 canary feed 永远不推进——
`publish-canary-updater-feed.yaml` 留了 `workflow_dispatch`（输入 tag）作为补救。

**发布后必须同步 Homebrew tap**，否则 brew 用户升不上来。改
[`nagi-studio/homebrew-ypm`](https://github.com/nagi-studio/homebrew-ypm) 的
`Formula/ypm.rb`：`version`、两条下载 URL 里的 tag，以及 `ypm-macos-aarch64` 和
`ypm-linux-x64` 两个 `sha256`（`shasum -a 256 <产物>`）。本仓库 `Formula/ypm.rb`
是模板，永远保持 `0.0.0` 占位，不要在这里填真版本号。

`ypm update` 判断 brew 用户该不该升级时读的是 tap 里的 formula 版本，不是 GitHub
Release 的 tag——tap 没同步就等于没发版，用户不会收到误报的升级提示，但也永远升不上来。
v0.9.0 就漏过一次：GitHub 标了 latest，tap 还停在 0.8.0。

**发布前必须手写 release 正文**，不能只留自动生成的 Full Changelog 链接。
仓库没有 CHANGELOG 文件，变更记录只存在于 release 正文里。格式照 v0.6.2 / v0.6.3：
一段 `## 修复`，用户视角的中文条目（说"能拖动窗口了"，不说"补了 drag-region 属性"），
末尾保留自动追加的 Full Changelog 那一行、不要自己再写一遍（v0.6.2 就重了）。

## 技术栈

Vue 3.5 + Pinia 4 + Vue Router 4 + TypeScript 6.0 + Vite 7 + Tauri 2，包管理用 bun。
Vue 组件保留选项式 API，统一使用
`<script lang="ts">` + `defineComponent`。

TypeScript 开启严格模式、`exactOptionalPropertyTypes` 和
`noUncheckedIndexedAccess`；外部数据先以 `unknown` 接收并缩窄，纯类型依赖使用
`import type`，复杂 props 使用 `PropType`。禁止用新增 `any`、`@ts-ignore` 或
`@ts-expect-error` 绕过类型检查。

渲染构建统一使用 `vite.config.mjs`。桌面主进程是 Rust，不存在第二套 JavaScript
桌面运行时或构建配置。

## 架构要点

Tauri 主进程入口是 `src-tauri/src/main.rs`，负责窗口、托盘、快捷键、单实例和 Sidecar
生命周期。`src-tauri/sidecar/` 会编译成各平台独立的 Rust 可执行文件，负责网易云 API、
托管渲染产物、同源 `/api` 代理和 UNM。正式版页面来自 `http://127.0.0.1:28232`；
`12754` API 端口在 dev 和 release 都会监听。

网易云业务（登录、搜索、歌单/专辑/歌手详情、播放源、歌词、收藏、每日推荐、私人 FM）
收在 `src-tauri/core/src/ncm.rs`，GUI 与 TUI 共用同一份实现：Sidecar 用 `/native/*`
类型化端点把它暴露给渲染层（`src/services/*.ts` 负责适配回组件形状），TUI 直接调。
`src-tauri/sidecar/src/ncm.rs` 只剩 manifest 驱动的通用转发壳（45 条路由，
`src/sidecar-route-manifest.json` 是唯一事实来源，改动要同步 `FRONTEND_ROUTE_COUNT`
和 `test/sidecarRouteManifest.test.ts` 的计数）。新迁端点照 `/native/*` 模板：
cookie + realIP + proxy 必须透传，逐行降级而不是整页报错。

**生产模式不走 `app://` 协议**，而是加载 Sidecar 的 loopback HTTP 页面。
dev 的 Vite server 也配了 `/api` 同源代理指向 12754 —— 这个不能省，否则跨端口属于跨站，登录 cookie 会被
WebView 的 SameSite 策略丢掉，表现为头像不刷新、library 空。

Windows 专属功能都在 `src-tauri/src/`：任务栏吸附 `taskbar_docking/`（`model.rs` 是纯状态机，
其余是 UI Automation 探测和窗口操作）、Win32 封装 `win32.rs`、QQ 音乐快速登录
`qq_music_login.rs`。UNM 的 QQ 音源是 `src-tauri/core/src/unm/qq.rs` 自己实现的，
不再依赖 `unm_engine_qq`。

迷你播放器做在 `src/views/lyrics.vue` 里：窗口高 < 340（`isBarWindowSize`）才切成
紧凑播放条；宽 < 620 或高 < 340（`isMiniWindowSize`）时 `src/App.vue` 自动切到歌词页，
窄而高的窗口保持完整播放器视图。两个判定都在 `src/utils/miniWindow.ts`，语义不能合并。窗口、菜单栏封面/歌词、全局快捷键和
Discord Rich Presence 都由 `src-tauri/src/` 的 Rust 实现。

## 数据目录（容易搞错）

dev 是未打包的二进制，WebKit 按进程名 `yesplaymusic-tauri` 存数据；正式版按 bundle id
`fun.nagi.yokaze` 存。两边的 cookie、localStorage 设置、IndexedDB 歌曲缓存
**全部不共用**，dev 要单独登录。

- dev：`~/Library/WebKit/yesplaymusic-tauri/`、`~/Library/Caches/yesplaymusic-tauri/`、
  `~/Library/HTTPStorages/yesplaymusic-tauri.binarycookies`，清 dev 数据只删这三处
- 正式版：同样三处，名字换成 `fun.nagi.yokaze`
- v0.13 及更早的 bundle id 是从 Electron 版继承的 `com.electron.yesplaymusic`。
  `src-tauri/src/identity_migration.rs` 在建窗口前把旧 id 下的数据整目录 rename 过来
  （WebKit 的 origin 目录名由目录内的 salt 算出，整目录搬才认得），旧版还开着就把这次启动交给它。
  ypm 的 `ctl.rs` 和 GUI 的控制 socket 同时认新旧两个路径，因为 brew 版 ypm 和 GUI 不同步升级
- `~/Library/Application Support/YesPlayMusic/` 是旧 Electron 版的数据，
  `legacy_renderer_data.rs` 首次启动时从这里迁移，不要删

改名 Yokaze 时**故意保留**了这些旧名字，它们是数据位置或内部协议，不是品牌，改了会丢数据：
`mainBinaryName` `yesplaymusic-tauri`、sidecar 二进制名、
IndexedDB 库名 `yesplaymusic`（`src/utils/db.ts`）、`X-YesPlayMusic-*` 内部 header，
以及 crate 名 `yesplaymusic-core` / `yesplaymusic-tui`（只是内部名字，暂未改）。
`ypm` 读作 Yokaze Player for Music，不要再写成 YesPlayMusic 的缩写。
Windows 安装器的 `src-tauri/windows/installer-hooks.nsh` 会静默卸掉旧名下的安装（不删数据），
因为 NSIS 的安装目录和卸载项按 productName 派生。

## 已知的坑

1. `src/ncmModDef.cjs` 是刻意保留的 Bun 参考实现 CommonJS 边界，必须静态 `import`，
   让 differential oracle 能收集网易云 API 路由；正式安装包只运行 Rust Sidecar。
2. `vite-plugin-svg-icons` 只在 dev server 启动时扫一遍 `src/assets/icons`，
   新加的 svg 要重启 dev 才会进 sprite，否则图标位置是空白。
3. `.player` 上有 `backdrop-filter`，超出它上边界的子元素会被裁掉 —— 进度条上的角色
   容易缺一块头。
4. 单实例锁：`/Applications` 里的正式版开着时，新起的实例会把焦点交给已有窗口后退出，
   看起来像打包失败。测试前先退掉。
5. 卸载 brew cask 时**不要加 `--zap`**，会连数据目录一起删。
6. macOS 上 `cp` 覆盖已存在的可执行文件会让它的代码签名失效，运行时被直接
   SIGKILL（表现为「什么都不输出、exit 137」）。换 `mv`（原子 rename）或
   `codesign -s - --force`；`ypm update` 的临时文件 + rename 正是为此。
7. `data-tauri-drag-region` 只在按下的元素**本身**带标记时才拖窗口（子元素不继承），
   所以每层空白的布局容器都要带；而 macOS 上在 `overflow: auto` 的滚动区里按下去根本
   拖不动窗口，歌词滚动区靠 `pointer-events: none`（行上再设回 `auto`）让空白穿透到外层。
8. ypm 启动时要向终端查询图形能力，之后会清扫残留回应（否则变成幽灵按键），
   这一扫会吃掉那一刻的真实按键——启动后第一次按键偶尔没反应是这个原因。

## 约定

- 代码注释使用精简英文，只解释必要的“为什么”
- 提交信息标题是 `<emoji> <类型>: <中文描述>`，例如 `🐛 fix: 迷你播放条双击不再最大化`。
  类型与 emoji 一一对应，白名单和规则在 `scripts/verify-commit-message.mjs`，
  `.githooks/commit-msg` 会拦下不合规的标题（rebase / merge 进行中自动放行）。
  正文用中文说清动机和影响
- 只用 `master` 一个分支，直接在主仓库目录里改，不开额外的 worktree 或分支
- 上游是 [qier222/YesPlayMusic](https://github.com/qier222/YesPlayMusic)，本地没配 remote，
  要同步时 `git remote add upstream https://github.com/qier222/YesPlayMusic.git`
- 外部 PR 需要大改时，不往对方分支推：自己提交，标题末尾带 `(#N, 作者)`，正文加
  `Co-authored-by: 作者 <id+作者@users.noreply.github.com>`，推上 master 后在 PR 里附提交链接致谢并关闭
- 终端前端一律写 **TUI**（`ypm TUI`），中文文案里也不要翻成「终端版」

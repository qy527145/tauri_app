# {{ project-name }}

基于 Tauri 2 + React + Vite 的桌面应用。目录沿用 `bun create tauri-app` 的官方布局，
在其上补齐类型化 IPC、统一错误、日志、单实例、窗口状态记忆、CSP 与三平台发布流水线。

## 快速开始

```bash
bun run init    # 安装依赖 + 从 .env.example 生成 .env
bun run dev     # 打开桌面窗口，前端 HMR，Rust 改动自动重编
```

前置依赖见 [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)
（Windows 需要 WebView2 与 MSVC，Linux 需要 WebKitGTK）。

## 构建

```bash
bun run build        # 当前平台安装程序
bun run build:debug  # 调试包：保留 devtools 与 sourcemap
```

产物在 `src-tauri/target/release/bundle/`：

| 平台    | 产物                        |
| ------- | --------------------------- |
| Windows | `nsis/*-setup.exe`          |
| macOS   | `macos/*.app`、`dmg/*.dmg`  |
| Linux   | `deb/`、`rpm/`、`appimage/` |

安装包无法跨平台构建（各平台打包工具链只在本机可用）。三平台同时出包交给 CI，见下面「发版」。

### 图标

```bash
bun run icon ./app-icon.png   # 1024x1024 PNG → src-tauri/icons/ 全套
```

## 发版

```bash
bun run upgrade patch          # 改版本号（package.json + Cargo.toml）→ 提交 → 打 tag v0.1.1
bun run upgrade patch --push   # 同上，并推送分支与 tag，立刻触发 CI
bun run upgrade 0.2.0          # 指定版本；也可以 minor / major
```

推 `v*` tag 会触发 `.github/workflows/release.yml`：先跑 `check` + `test`，再在三台构建机上各出一份安装包
（Windows NSIS `.exe`、macOS Apple Silicon `.dmg`、Linux `.deb`），建 GitHub Release 并挂上去。
流水线会校验 tag 与两处版本号一致，不一致直接失败。

手动触发：Actions → **Release** → Run workflow，`tag` 填 `v0.1.2`；填 `dry-run` 只构建不发版，适合先验证流水线。

Release 建不出来报 `403` 时，说明仓库把 `GITHUB_TOKEN` 限制成只读：补一个带 `repo` 权限的 PAT 存成
Secrets 里的 `RELEASE_TOKEN`，流水线会自动改用它。

## 应用内自更新

启动 5 秒后静默查一次 GitHub Release，有新版就在首页底部出现一条「有新版 x.y.z · 立即更新」；
点下去会下载对应平台的安装包、交给系统安装、然后自动重启到新版。也可以随时点「检查更新」手动查。

- **仓库坐标**来自 `src-tauri/Cargo.toml` 的 `repository`，`bun run init` 会按 git 的 `origin` 自动填好。
  没填（或没有 GitHub 远端）时更新功能静默关闭。
- **GitHub 加速**：某些地区 `github.com` / `api.github.com` 直连不通。每次请求都把官方源与几家加速站
  （`gh-proxy.com` / `ghproxy.net` / `ghfast.top`）**并发**发出，谁先回就用谁，之后记住这条线路。
  地址是写死的，不读任何远程清单。`GITHUB_ACCEL=0` 可关闭，`GITHUB_PROXY` 可指定代理。
- **各平台装法**：Windows 等本进程退出后静默跑 NSIS `/S`；macOS 挂载 dmg 后替换当前 `.app`；
  Linux 用 `pkexec dpkg -i`。安装包没有签名校验，所以装之前会把文件名与大小显示给用户。
- 代码在 `src-tauri/src/update/`（不依赖 Tauri，可单测）与 `src/lib/updater.ts`（全局状态）。
  不用它的话删掉这两处、`commands/update.rs` 与 `components/update-notice.tsx` 即可。

## 目录结构

```
src/                 React 前端
  lib/ipc.ts         类型化的 Rust 命令封装（唯一调用 invoke 的地方）
  lib/log.ts         前端日志 → 与 Rust 同一份日志文件
  lib/updater.ts     自更新的全局状态（进度、发现新版）
  components/ui/     共享组件
src-tauri/           Rust / Tauri
  src/commands/      #[tauri::command]，按领域分文件
  src/update/        自更新：查 Release、多源下载、按平台安装
  src/error.rs       AppError → { code, message }
  src/config.rs      环境变量
  capabilities/      前端权限清单
scripts/init.ts      一键初始化
scripts/upgrade.ts   发版：改版本号 + 提交 + 打 tag
```

## 内置能力

| 能力       | 实现                                                                     |
| ---------- | ------------------------------------------------------------------------ |
| 类型化 IPC | `src/lib/ipc.ts`，命令名与参数集中定义，错误统一为 `CommandError`        |
| 统一错误   | Rust `AppError` 序列化为 `{ code, message }`，前端按 `code` 分支         |
| 日志       | `tauri-plugin-log`：stdout + 系统日志目录，5 MB 轮转；前端日志汇入同一处 |
| 单实例     | 重复启动时唤起已有窗口                                                   |
| 窗口状态   | 记住位置、尺寸、最大化状态                                               |
| 安全       | 收紧的 CSP，capabilities 最小授权                                        |
| 发版       | `bun run upgrade` 一条命令改版本、打 tag；CI 三平台出包建 Release        |
| 自更新     | 启动时查 GitHub Release，一键下载安装；内置 GitHub 加速                  |

## 配置

`.env` 只在 debug 构建里加载；发布的安装包使用 `config.rs` 中的默认值。密钥放 `.env.local`（不会被提交）。

| 变量                | 默认   | 说明                                             |
| ------------------- | ------ | ------------------------------------------------ |
| `RUST_LOG`          | `info` | 日志过滤，`<级别>,<target>=<级别>`               |
| `GITHUB_ACCEL`      | `1`    | 从 GitHub 下载时允许走加速站；`0` 只试官方源     |
| `GITHUB_PROXY`      | 空     | 显式代理，如 `http://127.0.0.1:7897`；空 = 直连  |
| `AUTO_CHECK_UPDATE` | `1`    | 启动后自动查一次新版本；`0` 只剩手动「检查更新」 |

## 用模板创建新项目

```bash
cargo generate --git <this-repo-url> --name my-app
cd my-app
git init && git remote add origin https://github.com/<you>/my-app.git
bun run init      # 装依赖、生成 .env，并把仓库坐标写进 Cargo.toml（自更新要用）
```

生成时会询问 bundle identifier（如 `com.yourname.myapp`），它决定安装路径与数据目录，发布后不可更改。

## License

MIT

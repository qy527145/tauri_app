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

安装包无法跨平台构建（各平台打包工具链只在本机可用）。三平台同时出包用
`.github/workflows/release.yml`：推送 `v*` tag 后自动构建并挂到草稿 Release，macOS 同时出 arm64 与 x64。

### 图标

```bash
bun run icon ./app-icon.png   # 1024x1024 PNG → src-tauri/icons/ 全套
```

## 目录结构

```
src/                 React 前端
  lib/ipc.ts         类型化的 Rust 命令封装（唯一调用 invoke 的地方）
  lib/log.ts         前端日志 → 与 Rust 同一份日志文件
  components/ui/     共享组件
src-tauri/           Rust / Tauri
  src/commands/      #[tauri::command]，按领域分文件
  src/error.rs       AppError → { code, message }
  src/config.rs      环境变量
  capabilities/      前端权限清单
scripts/init.ts      一键初始化
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

## 配置

`.env` 只在 debug 构建里加载；发布的安装包使用 `config.rs` 中的默认值。密钥放 `.env.local`（不会被提交）。

| 变量       | 默认   | 说明                               |
| ---------- | ------ | ---------------------------------- |
| `RUST_LOG` | `info` | 日志过滤，`<级别>,<target>=<级别>` |

## 用模板创建新项目

```bash
cargo generate --git <this-repo-url> --name my-app
cd my-app
bun run init
```

生成时会询问 bundle identifier（如 `com.yourname.myapp`），它决定安装路径与数据目录，发布后不可更改。

## License

MIT

# AGENTS.md

给 AI Agent 和人类协作者的单一事实来源。项目约束以本文件为准；命令只有这里列出的才是「官方」的。

## 这是什么

Tauri 2 桌面应用模板：Rust 负责系统能力与业务逻辑，React 前端由 Vite 构建后打进安装包。
目录保持 `bun create tauri-app` 的官方布局（`src/` + `src-tauri/`），只在其上补齐工程化。

## 目录

| 路径                        | 职责                                                              |
| --------------------------- | ----------------------------------------------------------------- |
| `src/`                      | React 19 + Vite 8 + Tailwind v4 前端                              |
| `src/lib/ipc.ts`            | **唯一**调用 `invoke` 的地方：类型化命令 + `CommandError`         |
| `src/lib/log.ts`            | 前端日志，写进与 Rust 同一份日志文件                              |
| `src/lib/updater.ts`        | 自更新的全局状态；界面只读它，不自己订阅事件                      |
| `src/components/ui/`        | shadcn/ui 风格组件                                                |
| `src-tauri/src/lib.rs`      | `tauri::Builder`：插件、状态、命令注册                            |
| `src-tauri/src/commands/`   | 按领域分模块的 `#[tauri::command]`，在 `commands/mod.rs` 汇总     |
| `src-tauri/src/update/`     | 自更新：查 Release、GitHub 多源加速下载、按平台安装；不依赖 Tauri |
| `src-tauri/src/config.rs`   | **唯一**读取环境变量的地方                                        |
| `src-tauri/src/error.rs`    | `AppError` → 前端收到的 `{ code, message }`                       |
| `src-tauri/capabilities/`   | 前端可调用的权限清单，最小授权                                    |
| `src-tauri/tauri.conf.json` | 窗口、CSP、打包配置                                               |
| `.github/workflows/`        | CI：push/PR 跑检查，`v*` tag 三平台出包建 Release                 |
| `scripts/upgrade.ts`        | 发版：改版本号 → 提交 → 打 tag（`--push` 触发 CI）                |
| `.env`                      | 仅 debug 构建加载的本地默认值                                     |

## 命令

```bash
bun run init         # 一键安装依赖 + 准备 .env + 登记仓库坐标（幂等）
bun run dev          # 启动桌面应用：Vite HMR + Rust 改动自动重编重启
bun run build        # 打包当前平台安装程序 → src-tauri/target/release/bundle/
bun run build:debug  # 带 devtools 与 sourcemap 的调试包
bun run check        # biome check + tsc --noEmit + cargo fmt --check + clippy -D warnings
bun run test         # bun test + cargo test
bun run lint         # biome check
bun run fmt          # biome --write + cargo fmt
bun run icon <png>   # 由一张 1024x1024 PNG 生成全部平台图标
bun run upgrade <patch|minor|major|x.y.z> [--push]   # 发版：改版本号 + 提交 + 打 tag
```

`web:dev` / `web:build` 只给 `tauri.conf.json` 的 `beforeDevCommand` / `beforeBuildCommand` 用，平时不必手动跑。

## 规则

1. **新增命令走三步**：`commands/<领域>.rs` 写 `#[tauri::command] async fn` → 在 `commands::handler()` 注册 →
   在 `src/lib/ipc.ts` 的 `commands` 补类型化封装。组件里不要直接 `invoke("...")`。
2. **命令一律 `async`，返回 `AppResult<T>`**。同步命令跑在主线程，稍慢就会卡住窗口。
   新增错误码时同步 `AppError::code()` 与 `ipc.ts` 的 `ErrorCode`。
3. **跨语言类型手写并保持同步**：Rust 结构体用 `#[serde(rename_all = "camelCase")]`，
   与 `ipc.ts` 里的 interface 字段一一对应，改动时两边同改。
4. **环境变量只从 `Config::from_env()` 读**，且必须有默认值——发给用户的安装包没有 `.env`。
   新增变量同时更新：`config.rs`、`.env`、`.env.example`（带一行注释）、README 表格。
5. **新增插件**：`Cargo.toml` + `package.json` 两边装 → `lib.rs` 注册 → `capabilities/default.json` 授权。
   只在桌面端可用的插件放进 `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`
   并在 `lib.rs` 的 `#[cfg(desktop)]` 块里注册。
6. **CSP 不要放宽成 `null`**。需要加载外部资源时，在 `tauri.conf.json` 的 `csp` 里精确加域名。
7. 前端样式用 Tailwind v4 工具类 + `src/components/ui`；整页默认不可选中文本，
   需要可复制的区域加 `data-selectable`。
8. 不引入第二个前端构建器，不引入 node_modules 之外的全局安装（`tauri` CLI 走 devDependency）。
9. **版本号只用 `bun run upgrade` 改**，它同时改 `package.json` 与 `src-tauri/Cargo.toml`
   （`tauri.conf.json` 不写 `version`，沿用 Cargo 的）。CI 会校验 tag 与两处一致。
10. **Release 每个平台只放一份安装包**（NSIS exe / arm64 dmg / amd64 deb）：应用内更新按后缀与架构挑文件，
    同一平台两份它选不出来。加目标时在 `release.yml` 的 matrix 加行，并确认 `update/asset.rs` 能区分。
11. **凡是从 GitHub 取东西都走 `update::net::get_first`**：它自带加速站并发与「记住可达线路」。
    加速站列表 `GITHUB_ACCELS` 是写死的，不要改成从远端读。
12. **模板文件里不能出现连续两个左花括号**（除了 `project-name` 这类占位符）：cargo-generate 用
    Liquid 渲染所有文件，JSX 的内联 `style` 对象字面量、注释里的示例都会让整个生成中止。
    内联样式先赋给变量再传；确实要原样保留的文件加进 `cargo-generate.toml` 的 `exclude`。

## 完成定义

改完代码后至少跑通：`bun run check` 与 `bun run test`。涉及打包配置时再跑 `bun run build` 确认安装包能生成。
模板本身的 `Cargo.toml` 里是 `{{project-name}}` 占位符，cargo 直接跑不了：验证 Rust 改动时先
`cargo generate --path . --name demo` 生成一个临时项目，在里面跑 `bun run init && bun run check && bun run test`。

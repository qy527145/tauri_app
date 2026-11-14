import { invoke } from "@tauri-apps/api/core";

/**
 * 前端与 Rust 的唯一通道。组件里不要直接 `invoke("...")`：
 * 命令名和参数写错只能在运行时发现，集中在这里就能靠类型兜住。
 *
 * 新增命令：Rust 侧写 `#[tauri::command]` 并注册到 `commands::handler()`，
 * 再在下面的 `commands` 里补一行。字段名与 Rust 的 serde 输出保持一致。
 */

/** 对应 `src-tauri/src/commands/app.rs` 的 `AppInfo`（camelCase）。 */
export interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
  os: string;
  arch: string;
  debug: boolean;
  uptimeSecs: number;
}

/** 对应 `src-tauri/src/error.rs` 的 `AppError::code()`。 */
export type ErrorCode = "invalid_input" | "update" | "internal";

// ---------------------------------------------------------------------------
// 自更新（`src-tauri/src/commands/update.rs`）
// ---------------------------------------------------------------------------

/** 下载进度事件名。与 Rust 的 `UPDATE_PROGRESS` 必须一致。 */
export const UPDATE_PROGRESS = "update://progress";

/** 启动后自动检查发现新版本的事件名。与 Rust 的 `UPDATE_AVAILABLE` 必须一致。 */
export const UPDATE_AVAILABLE = "update://available";

/** 对应 `src-tauri/src/update/mod.rs` 的 `ReleaseInfo`（camelCase）。 */
export interface ReleaseInfo {
  /** `v0.1.2`。 */
  tag: string;
  /** 去掉 `v` 的版本号。 */
  version: string;
  /** Release 页面，「更新说明」跳这里。 */
  htmlUrl: string;
  /** 这台机器要下的安装包文件名。 */
  assetName: string;
  /** 下载地址：后端按仓库名拼出来的，前端只回显。 */
  assetUrl: string;
  assetSize: number;
}

/** 对应 `commands/update.rs` 的 `UpdateCheck`。`release` 为 `null` 表示已是最新。 */
export interface UpdateCheck {
  current: string;
  release: ReleaseInfo | null;
}

/** 对应 `commands/update.rs` 的 `UpdateProgress`（`UPDATE_PROGRESS` 事件载荷）。 */
export type UpdateProgress =
  | { phase: "downloading"; downloaded: number; total: number | null }
  | { phase: "installing" }
  | { phase: "restarting" };

/** 所有命令失败时抛出的错误，`code` 用于分支，`message` 用于展示。 */
export class CommandError extends Error {
  override readonly name = "CommandError";

  constructor(
    readonly command: string,
    readonly code: ErrorCode | "unknown",
    message: string,
  ) {
    super(message);
  }
}

function isPayload(value: unknown): value is { code: ErrorCode; message: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as Record<string, unknown>).code === "string" &&
    typeof (value as Record<string, unknown>).message === "string"
  );
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (isPayload(error)) throw new CommandError(command, error.code, error.message);
    throw new CommandError(command, "unknown", String(error));
  }
}

export const commands = {
  appInfo: () => call<AppInfo>("app_info"),
  greet: (name: string) => call<string>("greet", { name }),
  /** 查最新 Release。没有新版时 `release` 为 `null`。 */
  checkUpdate: () => call<UpdateCheck>("check_update"),
  /**
   * 一键更新：查 → 下 → 装 → 退出并由安装脚本拉起新版。进度走 `UPDATE_PROGRESS` 事件。
   * 正常情况下进程会先退出，这个 Promise 的 resolve 前端看不到；reject 才是真的失败。
   */
  applyUpdate: () => call<void>("apply_update"),
  /** 取消正在进行的下载。没有下载在跑时什么也不做。 */
  cancelUpdate: () => call<void>("cancel_update"),
};

/** 不在 Tauri 里（比如直接用浏览器打开 Vite 地址）时，`invoke` 不可用。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

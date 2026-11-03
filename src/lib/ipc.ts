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
export type ErrorCode = "invalid_input" | "internal";

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
};

/** 不在 Tauri 里（比如直接用浏览器打开 Vite 地址）时，`invoke` 不可用。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

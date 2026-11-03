import * as log from "@tauri-apps/plugin-log";

import { isTauri } from "@/lib/ipc";

/**
 * 前端日志统一走这里：在 Tauri 里写进与 Rust 同一份日志文件，
 * 在浏览器里（直接打开 Vite 地址）退化为 console。
 */
export const logger = {
  debug: (message: string) => (isTauri() ? log.debug(message) : console.debug(message)),
  info: (message: string) => (isTauri() ? log.info(message) : console.info(message)),
  warn: (message: string) => (isTauri() ? log.warn(message) : console.warn(message)),
  error: (message: string) => (isTauri() ? log.error(message) : console.error(message)),
};

/** 把 Rust 侧日志也打到 WebView devtools 里，开发时前后端日志在一个地方看。 */
export async function attachConsole(): Promise<void> {
  if (isTauri() && import.meta.env.DEV) {
    await log.attachConsole();
  }
}

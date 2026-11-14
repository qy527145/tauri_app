import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import {
  commands,
  isTauri,
  type ReleaseInfo,
  UPDATE_AVAILABLE,
  UPDATE_PROGRESS,
  type UpdateProgress,
} from "@/lib/ipc";
import { logger } from "@/lib/log";

/**
 * 自更新的前端状态。放在模块级而不是组件里：更新是全局的一件事（进度只有一份、
 * 同一时刻只允许一次下载），多处各订阅一遍事件会在进度上互相打架。
 *
 * 用法：组件里 `const { release, state } = useUpdater()`，再按需调
 * `checkForUpdate()` / `startUpdate()` / `cancelUpdate()`。
 */

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "downloading"; downloaded: number; total: number | null }
  /** 下载完了，正在交给系统装。 */
  | { kind: "installing" }
  /** 等本进程退出、由安装脚本拉起新版。 */
  | { kind: "restarting" }
  | { kind: "error"; message: string };

interface Snapshot {
  /** 查到的新版本；`null` = 没有或还没查。 */
  release: ReleaseInfo | null;
  /** 最近一次手动检查是否确认了「已是最新」。 */
  upToDate: boolean;
  state: UpdateState;
}

let snapshot: Snapshot = { release: null, upToDate: false, state: { kind: "idle" } };
const watchers = new Set<(s: Snapshot) => void>();
let subscribed = false;

function set(patch: Partial<Snapshot>) {
  snapshot = { ...snapshot, ...patch };
  for (const w of watchers) w(snapshot);
}

/** 订阅后端事件，整个应用生命周期只做一次。 */
function ensureSubscribed() {
  if (subscribed || !isTauri()) return;
  subscribed = true;
  void listen<ReleaseInfo>(UPDATE_AVAILABLE, ({ payload }) => {
    set({ release: payload, upToDate: false });
  });
  void listen<UpdateProgress>(UPDATE_PROGRESS, ({ payload }) => {
    if (payload.phase === "downloading") {
      set({
        state: { kind: "downloading", downloaded: payload.downloaded, total: payload.total },
      });
    } else {
      set({ state: { kind: payload.phase } });
    }
  });
}

/** 手动检查。结果同时更新 `release` 与 `upToDate`。 */
export async function checkForUpdate(): Promise<void> {
  set({ state: { kind: "checking" } });
  try {
    const result = await commands.checkUpdate();
    set({ release: result.release, upToDate: result.release === null, state: { kind: "idle" } });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logger.warn(`check update failed: ${message}`);
    set({ state: { kind: "error", message } });
  }
}

/** 一键更新。正常情况下不会返回（进程退出），只有失败才回到 `error`。 */
export async function startUpdate(): Promise<void> {
  set({ state: { kind: "downloading", downloaded: 0, total: null } });
  try {
    await commands.applyUpdate();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logger.warn(`apply update failed: ${message}`);
    set({ state: { kind: "error", message } });
  }
}

export async function cancelUpdate(): Promise<void> {
  try {
    await commands.cancelUpdate();
  } catch {
    // 没有下载在跑。
  }
  set({ state: { kind: "idle" } });
}

/** 正在更新（下载 / 安装 / 重启），这期间所有更新入口都该锁住。 */
export function updating(s: UpdateState): boolean {
  return s.kind === "downloading" || s.kind === "installing" || s.kind === "restarting";
}

/** 进度百分比；总长未知时 `null`。 */
export function percent(s: UpdateState): number | null {
  if (s.kind !== "downloading" || !s.total) return null;
  return Math.min(100, Math.round((s.downloaded / s.total) * 100));
}

/** 组件里用：拿到实时的更新状态。 */
export function useUpdater(): Snapshot {
  const [s, setS] = useState(snapshot);
  useEffect(() => {
    ensureSubscribed();
    watchers.add(setS);
    setS(snapshot);
    return () => {
      watchers.delete(setS);
    };
  }, []);
  return s;
}

import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpCircle, ExternalLink, Loader2, RefreshCw, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  cancelUpdate,
  checkForUpdate,
  percent,
  startUpdate,
  updating,
  useUpdater,
} from "@/lib/updater";

/** 用户点过「稍后」的那一版，换版本后会重新提示。 */
const DISMISS_KEY = "update.dismissed";

function dismissed(version: string): boolean {
  try {
    return localStorage.getItem(DISMISS_KEY) === version;
  } catch {
    return false;
  }
}

function mb(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/**
 * 更新提示条：有新版时显示「有新版 x.y.z · 立即更新」，更新中变成进度。
 * 刻意做成一条安静的行而不是弹窗。没有新版时只有一个「检查更新」按钮。
 */
export function UpdateNotice() {
  const { release, upToDate, state } = useUpdater();
  const [skipped, setSkipped] = useState(false);

  const busy = updating(state);
  const pct = percent(state);
  // 不要在 JSX 里写内联的 style 对象字面量（连续两个左花括号）：
  // 模板经 cargo-generate 渲染时会被当成 Liquid 语法，整个生成会中止。
  const barStyle = { width: `${pct ?? 100}%` };

  if (!release) {
    return (
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <Button
          variant="ghost"
          size="sm"
          disabled={state.kind === "checking"}
          onClick={() => void checkForUpdate()}
        >
          {state.kind === "checking" ? (
            <Loader2 className="animate-spin" />
          ) : (
            <RefreshCw className="size-3.5" />
          )}
          检查更新
        </Button>
        {upToDate && state.kind === "idle" && <span>已经是最新版本</span>}
        {state.kind === "error" && <span className="text-red-400">{state.message}</span>}
      </div>
    );
  }

  if (!busy && (skipped || dismissed(release.version))) return null;

  const skip = () => {
    setSkipped(true);
    try {
      localStorage.setItem(DISMISS_KEY, release.version);
    } catch {
      // 存不下就只是这次运行里不再提示。
    }
  };

  return (
    <div className="flex items-center gap-3 rounded-lg border bg-card px-3 py-2 text-xs">
      <ArrowUpCircle className="size-4 shrink-0 text-muted-foreground" />
      {busy ? (
        <>
          <span className="shrink-0">
            {state.kind === "downloading"
              ? `正在下载 ${release.version}`
              : state.kind === "installing"
                ? "正在安装"
                : "即将重启"}
          </span>
          <span className="h-1 min-w-24 flex-1 overflow-hidden rounded-full bg-muted">
            <span
              className="block h-full bg-primary transition-[width] duration-200"
              style={barStyle}
            />
          </span>
          <span className="shrink-0 tabular-nums text-muted-foreground">
            {state.kind === "downloading"
              ? state.total
                ? `${pct}% · ${mb(state.downloaded)} / ${mb(state.total)}`
                : mb(state.downloaded)
              : "应用会自动重新打开"}
          </span>
          {state.kind === "downloading" && (
            <Button size="sm" variant="ghost" onClick={() => void cancelUpdate()}>
              取消
            </Button>
          )}
        </>
      ) : (
        <>
          <span className="min-w-0 flex-1 truncate">
            有新版 <span className="font-mono">{release.version}</span>
            {release.assetSize > 0 && (
              <span className="text-muted-foreground">（{mb(release.assetSize)}）</span>
            )}
            {state.kind === "error" && (
              <span className="ml-2 text-red-400" data-selectable>
                {state.message}
              </span>
            )}
          </span>
          <Button
            size="sm"
            variant="ghost"
            title="更新说明"
            onClick={() => void openUrl(release.htmlUrl).catch(() => {})}
          >
            <ExternalLink className="size-3.5" />
          </Button>
          <Button size="sm" onClick={() => void startUpdate()}>
            立即更新
          </Button>
          <Button size="sm" variant="ghost" title="稍后再说" onClick={skip}>
            <X className="size-3.5" />
          </Button>
        </>
      )}
    </div>
  );
}

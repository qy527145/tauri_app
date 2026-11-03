import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpRight, Cpu, Loader2 } from "lucide-react";
import { type FormEvent, useCallback, useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { type AppInfo, CommandError, commands, isTauri } from "@/lib/ipc";
import { logger } from "@/lib/log";

type Info =
  | { kind: "loading" }
  | { kind: "ok"; data: AppInfo }
  | { kind: "error"; message: string };

export default function App() {
  const [info, setInfo] = useState<Info>({ kind: "loading" });
  const [name, setName] = useState("");
  const [reply, setReply] = useState<{ ok: boolean; text: string } | null>(null);
  const [pending, setPending] = useState(false);

  const refresh = useCallback(async () => {
    if (!isTauri()) {
      setInfo({ kind: "error", message: "当前不在 Tauri 窗口里：请用 `bun run dev` 启动。" });
      return;
    }
    try {
      setInfo({ kind: "ok", data: await commands.appInfo() });
    } catch (error) {
      setInfo({ kind: "error", message: String(error) });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function onGreet(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    try {
      setReply({ ok: true, text: await commands.greet(name) });
    } catch (error) {
      // CommandError.code 是稳定的分支依据；message 只用来展示。
      const text = error instanceof CommandError ? error.message : String(error);
      logger.warn(`greet failed: ${text}`);
      setReply({ ok: false, text });
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="mx-auto flex h-full max-w-2xl flex-col justify-center gap-8 px-8">
      <header className="space-y-2">
        <p className="text-sm text-muted-foreground">Tauri · Rust · Vite · React</p>
        <h1 className="text-4xl font-semibold tracking-tight">{"{{project-name}}"}</h1>
        <p className="text-muted-foreground">
          编辑 <code className="rounded bg-muted px-1.5 py-0.5 text-sm">src-tauri/src</code> 或{" "}
          <code className="rounded bg-muted px-1.5 py-0.5 text-sm">src</code> 开始开发。
        </p>
      </header>

      <section className="rounded-lg border bg-card p-6 text-sm">
        <p className="mb-3 flex items-center gap-2 font-mono text-xs uppercase tracking-wider text-muted-foreground">
          <Cpu className="size-3.5" /> invoke("app_info")
        </p>
        {info.kind === "loading" && <p className="text-muted-foreground">读取中…</p>}
        {info.kind === "error" && <p className="text-red-400">{info.message}</p>}
        {info.kind === "ok" && (
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 font-mono" data-selectable>
            <dt className="text-muted-foreground">version</dt>
            <dd>{info.data.version}</dd>
            <dt className="text-muted-foreground">tauri</dt>
            <dd>{info.data.tauriVersion}</dd>
            <dt className="text-muted-foreground">platform</dt>
            <dd>
              {info.data.os} / {info.data.arch}
            </dd>
            <dt className="text-muted-foreground">profile</dt>
            <dd>{info.data.debug ? "debug" : "release"}</dd>
          </dl>
        )}
      </section>

      <form onSubmit={onGreet} className="flex gap-2">
        <input
          value={name}
          onChange={(event) => setName(event.currentTarget.value)}
          placeholder="输入名字，调用 Rust 命令"
          className="h-9 flex-1 rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-primary/40"
        />
        <Button type="submit" disabled={pending}>
          {pending && <Loader2 className="animate-spin" />}
          调用 greet
        </Button>
      </form>
      {reply && (
        <p className={reply.ok ? "text-sm" : "text-sm text-red-400"} data-selectable>
          {reply.text}
        </p>
      )}

      <footer>
        <Button variant="ghost" size="sm" onClick={() => void openUrl("https://v2.tauri.app")}>
          Tauri 文档 <ArrowUpRight />
        </Button>
      </footer>
    </main>
  );
}

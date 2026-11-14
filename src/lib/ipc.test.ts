import { describe, expect, mock, test } from "bun:test";

const invoke = mock(async (_command: string, _args?: unknown): Promise<unknown> => undefined);
mock.module("@tauri-apps/api/core", () => ({ invoke }));

const { CommandError, commands } = await import("./ipc");

describe("commands", () => {
  test("透传命令名与参数", async () => {
    invoke.mockResolvedValueOnce("你好");
    await expect(commands.greet("Ferris")).resolves.toBe("你好");
    expect(invoke).toHaveBeenLastCalledWith("greet", { name: "Ferris" });
  });

  test("Rust 的 AppError 转成 CommandError", async () => {
    invoke.mockRejectedValueOnce({ code: "invalid_input", message: "名字不能为空" });
    const error = await commands.greet("").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(CommandError);
    expect(error).toMatchObject({
      command: "greet",
      code: "invalid_input",
      message: "名字不能为空",
    });
  });

  test("非约定形状的错误归为 unknown", async () => {
    invoke.mockRejectedValueOnce("window closed");
    const error = await commands.appInfo().catch((e: unknown) => e);
    expect(error).toMatchObject({ code: "unknown", message: "window closed" });
  });
});

describe("update commands", () => {
  test("checkUpdate 透传后端结果", async () => {
    invoke.mockResolvedValueOnce({ current: "0.1.0", release: null });
    await expect(commands.checkUpdate()).resolves.toEqual({ current: "0.1.0", release: null });
    expect(invoke).toHaveBeenLastCalledWith("check_update", undefined);
  });

  test("update 错误码保留", async () => {
    invoke.mockRejectedValueOnce({ code: "update", message: "已经是最新版本" });
    const error = await commands.applyUpdate().catch((e: unknown) => e);
    expect(error).toMatchObject({ command: "apply_update", code: "update" });
  });
});

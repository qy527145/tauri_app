import { existsSync } from "node:fs";
import { copyFile, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { parseGithubRepo } from "./lib/repo.ts";

/**
 * 一键初始化：检查工具链 -> 安装依赖 -> 准备 .env -> 登记仓库坐标 -> 提示下一步。
 * 幂等，可重复执行。
 */

const root = join(import.meta.dir, "..");

/**
 * 应用内「检查更新」要知道去哪个 GitHub 仓库查 Release。从 origin 推出 owner/repo
 * 写进 Cargo.toml 的 `repository`，Rust 侧编译期通过 `CARGO_PKG_REPOSITORY` 读到。
 * 没有 GitHub 远端时留空，更新功能静默关闭，其它一切照常。
 */
async function registerRepository() {
  const remote = Bun.spawnSync({
    cmd: ["git", "remote", "get-url", "origin"],
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
  });
  const repo = remote.exitCode === 0 ? parseGithubRepo(remote.stdout.toString()) : null;
  if (!repo) {
    console.log(
      "\n还没有 GitHub 的 origin 远端：应用内「检查更新」暂不可用。\n  加好远端后再跑一次 bun run init 即可登记。",
    );
    return;
  }
  const cargoPath = join(root, "src-tauri", "Cargo.toml");
  const cargo = await readFile(cargoPath, "utf8");
  const line = `repository = "https://github.com/${repo}"`;
  if (cargo.includes(line)) {
    console.log(`\n仓库坐标已登记：${repo}`);
    return;
  }
  const next = cargo.replace(/^repository\s*=.*$/m, line);
  if (next === cargo) {
    console.warn("⚠ src-tauri/Cargo.toml 里没有 repository 行，请手动在 [package] 下加上：");
    console.warn(`  ${line}`);
    return;
  }
  await writeFile(cargoPath, next);
  console.log(`\n已把仓库坐标写进 src-tauri/Cargo.toml：${repo}`);
}

function check(cmd: string, args: string[], hint: string): string | null {
  try {
    const result = Bun.spawnSync({ cmd: [cmd, ...args], stdout: "pipe", stderr: "pipe" });
    if (result.exitCode === 0) {
      return result.stdout.toString().trim().split("\n")[0] ?? "";
    }
  } catch {
    // 命令不存在时 spawnSync 直接抛错，与非零退出同样处理。
  }
  console.warn(`⚠ ${cmd} 不可用：${hint}`);
  return null;
}

console.log("检查工具链…");
const cargo = check("cargo", ["--version"], "安装 Rust: https://rustup.rs");
console.log(`  bun   ${Bun.version}`);
console.log(`  cargo ${cargo ?? "(缺失)"}`);
if (process.platform === "linux") {
  console.log("  Linux 还需要 WebKitGTK 等系统库：https://v2.tauri.app/start/prerequisites/#linux");
}

console.log("\n安装依赖…");
const install = Bun.spawnSync({
  cmd: ["bun", "install"],
  cwd: root,
  stdio: ["inherit", "inherit", "inherit"],
});
if (install.exitCode !== 0) {
  throw new Error("依赖安装失败");
}

const envPath = join(root, ".env");
if (!existsSync(envPath)) {
  await copyFile(join(root, ".env.example"), envPath);
  console.log("\n已从 .env.example 创建 .env");
} else {
  console.log("\n.env 已存在，保持不变");
}

await registerRepository();

console.log(`
完成。下一步：

  bun run dev       # 启动桌面应用（Vite HMR + Rust 自动重编）
  bun run check     # lint + 类型检查 + clippy
  bun run build     # 打包安装程序 → src-tauri/target/release/bundle/
  bun run upgrade   # 发版：改版本号 + 提交 + 打 tag（加 --push 直接触发 CI）
`);

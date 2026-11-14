import { existsSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { parseGithubRepo } from "./lib/repo.ts";
import { resolveTarget } from "./lib/version.ts";

/**
 * 一条命令发版：改版本号 → 提交 → 打 tag →（可选）推送触发 CI。
 *
 *   bun run upgrade                  # 打印当前版本
 *   bun run upgrade patch            # 0.1.0 → 0.1.1（也可以 minor / major）
 *   bun run upgrade 0.2.0            # 指定版本
 *   bun run upgrade patch --push     # 顺便 push 分支与 tag，立刻触发 release.yml
 *
 * 版本号有两处，手改容易漏：
 *   - package.json            `version`
 *   - src-tauri/Cargo.toml    `[package] version`（tauri.conf.json 不写 version 时沿用它，
 *                             安装包文件名与应用内「检查更新」比对的都是它）
 *
 * 默认只提交、打 tag，不推送 —— 推不推由你决定。推 `v*` tag 会触发
 * .github/workflows/release.yml，三平台出安装包并建 GitHub Release。
 */

const root = join(import.meta.dir, "..");
const files = {
  pkg: join(root, "package.json"),
  cargo: join(root, "src-tauri", "Cargo.toml"),
  lock: join(root, "src-tauri", "Cargo.lock"),
};

const args = process.argv.slice(2);
const push = args.includes("--push");
const arg = args.find((a) => !a.startsWith("--"));

const pkg = JSON.parse(await readFile(files.pkg, "utf8")) as { version: string };
const current = pkg.version;

if (!arg) {
  console.log(current);
  process.exit(0);
}

const target = resolveTarget(current, arg);
const tag = `v${target}`;

if (target === current) {
  console.error(`已经是 ${current}，没有要改的版本号`);
  process.exit(1);
}

/** 同步执行 git。`capture` 时返回 stdout，否则输出直通终端。 */
function git(cmd: string[], opts: { capture?: boolean } = {}): { code: number; out: string } {
  const proc = Bun.spawnSync({
    cmd: ["git", ...cmd],
    cwd: root,
    stdout: opts.capture ? "pipe" : "inherit",
    stderr: opts.capture ? "pipe" : "inherit",
  });
  return {
    code: proc.exitCode,
    out: opts.capture ? (proc.stdout?.toString().trim() ?? "") : "",
  };
}

// 先把能提前发现的问题都查完，再碰文件 —— 改了一半再回滚最麻烦。
if (git(["rev-parse", "--verify", "--quiet", `refs/tags/${tag}`], { capture: true }).code === 0) {
  console.error(`tag ${tag} 已存在；换个版本号，或先 git tag -d ${tag}`);
  process.exit(1);
}

const dirty = git(["status", "--porcelain"], { capture: true }).out;
if (dirty) {
  console.error(`工作区还有未提交的改动，先提交或 stash 再发版：\n${dirty}`);
  process.exit(1);
}

const branch = git(["rev-parse", "--abbrev-ref", "HEAD"], { capture: true }).out;
const remote = git(["remote", "get-url", "origin"], { capture: true });
const repo = remote.code === 0 ? parseGithubRepo(remote.out) : null;
if (push && !repo) {
  console.error("找不到 GitHub 的 origin 远端，没法 --push");
  process.exit(1);
}

/** 只替换第一处匹配；替换不到就报错，静默跳过正是「漏改一处」的来源。 */
async function replaceIn(file: string, pattern: RegExp, label: string) {
  const text = await readFile(file, "utf8");
  if (!pattern.test(text)) throw new Error(`${label} 里没找到版本号`);
  await writeFile(
    file,
    text.replace(pattern, (_m, pre: string) => `${pre}${target}"`),
  );
}

await replaceIn(files.pkg, /("version":\s*")[^"]+"/, "package.json");
await replaceIn(
  files.cargo,
  /(\[package\][^[]*?\nversion\s*=\s*")[^"]+"/,
  "src-tauri/Cargo.toml [package]",
);

const changed = ["package.json", "src-tauri/Cargo.toml"];

// Cargo.lock 里也记着本包的版本号；不更新的话下一次构建会改出一份 diff。
// 还没构建过（没有 Cargo.lock）时跳过。
if (existsSync(files.lock)) {
  const update = Bun.spawnSync({
    cmd: ["cargo", "update", "--manifest-path", files.cargo, "--workspace", "--offline"],
    cwd: root,
    stdout: "ignore",
    stderr: "pipe",
  });
  if (update.exitCode === 0) {
    changed.push("src-tauri/Cargo.lock");
  } else {
    console.warn(`⚠ cargo update 失败，Cargo.lock 没更新：${update.stderr.toString().trim()}`);
  }
}

if (git(["add", ...changed]).code !== 0 || git(["commit", "-m", `chore: 发布 ${tag}`]).code !== 0) {
  console.error(`\n提交失败：版本号已改成 ${target}，但没有提交，也没有打 tag。`);
  process.exit(1);
}

if (git(["tag", tag]).code !== 0) {
  console.error(`\n打 tag 失败：版本号已提交，但 ${tag} 没打上。`);
  process.exit(1);
}

console.log(`\n${current} → ${target}\n已提交：chore: 发布 ${tag}\n已打 tag：${tag}`);

if (push) {
  if (git(["push", "origin", branch]).code !== 0 || git(["push", "origin", tag]).code !== 0) {
    console.error("\n推送失败。本地提交与 tag 都在，网络好了之后手动：");
    console.error(`  git push origin ${branch} && git push origin ${tag}`);
    process.exit(1);
  }
  console.log(`\n已推送。流水线：https://github.com/${repo}/actions`);
} else {
  console.log(`\n推送后触发发布流水线：\n  git push origin ${branch} && git push origin ${tag}`);
  if (repo) console.log(`或下次直接：bun run upgrade ${arg} --push`);
}

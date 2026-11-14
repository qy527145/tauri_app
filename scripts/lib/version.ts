/**
 * 版本号工具。纯函数，单独放这里是为了能单测 —— upgrade 脚本本身只做文件读写。
 */

export type BumpKind = "major" | "minor" | "patch";

/** 只认 `x.y.z` 与带预发布后缀的 `x.y.z-...`。 */
const SEMVER = /^(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?$/;

/** 解析成数字段；不是 `x.y.z` 就返回 `null`。 */
export function parseVersion(raw: string): [number, number, number] | null {
  const match = SEMVER.exec(raw.trim());
  if (!match) return null;
  const [, major = "0", minor = "0", patch = "0"] = match;
  return [Number(major), Number(minor), Number(patch)];
}

/** 按语义化版本升一档。 */
export function bump(current: string, kind: BumpKind): string {
  const parsed = parseVersion(current);
  if (!parsed) throw new Error(`当前版本 ${current} 不是 x.y.z 形式`);
  const [major, minor, patch] = parsed;
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  return `${major}.${minor}.${patch + 1}`;
}

/**
 * 把命令行参数解析成目标版本：`major` / `minor` / `patch`，或直接写 `x.y.z`。
 * 看不懂就抛错 —— 静默按 patch 处理正是「发错版本」的来源。
 */
export function resolveTarget(current: string, arg: string): string {
  if (arg === "major" || arg === "minor" || arg === "patch") return bump(current, arg);
  if (SEMVER.test(arg.trim())) return arg.trim();
  throw new Error(`看不懂的版本参数：${arg}（用 major / minor / patch 或 x.y.z）`);
}

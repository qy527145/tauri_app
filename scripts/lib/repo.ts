/**
 * 从 git 远端地址里取出 `owner/repo`。
 *
 * 应用内「检查更新」需要它拼 GitHub API 与 Release 地址。`bun run init` 把它
 * 写进 `src-tauri/Cargo.toml` 的 `repository`，Rust 侧编译期读取。
 */

/** 认 HTTPS 与 SSH 两种写法；不是 github.com 或形状不对就返回 `null`。 */
export function parseGithubRepo(remote: string): string | null {
  const url = remote.trim();
  const patterns = [
    /^https?:\/\/github\.com\/([^/]+)\/([^/]+?)(?:\.git)?\/?$/,
    /^git@github\.com:([^/]+)\/([^/]+?)(?:\.git)?\/?$/,
    /^ssh:\/\/git@github\.com\/([^/]+)\/([^/]+?)(?:\.git)?\/?$/,
  ];
  for (const pattern of patterns) {
    const match = pattern.exec(url);
    if (match) {
      const [, owner = "", repo = ""] = match;
      return owner && repo ? `${owner}/${repo}` : null;
    }
  }
  return null;
}

import { describe, expect, test } from "bun:test";

import { parseGithubRepo } from "./repo.ts";

describe("parseGithubRepo", () => {
  test("HTTPS 写法", () => {
    expect(parseGithubRepo("https://github.com/owner/repo.git")).toBe("owner/repo");
    expect(parseGithubRepo("https://github.com/owner/repo")).toBe("owner/repo");
    expect(parseGithubRepo("https://github.com/owner/repo/")).toBe("owner/repo");
  });

  test("SSH 写法", () => {
    expect(parseGithubRepo("git@github.com:owner/repo.git")).toBe("owner/repo");
    expect(parseGithubRepo("ssh://git@github.com/owner/repo.git")).toBe("owner/repo");
  });

  test("其它主机与非仓库地址返回 null", () => {
    expect(parseGithubRepo("https://gitlab.com/owner/repo.git")).toBeNull();
    expect(parseGithubRepo("https://github.com/owner")).toBeNull();
    expect(parseGithubRepo("")).toBeNull();
  });
});

import { describe, expect, test } from "bun:test";

import { bump, parseVersion, resolveTarget } from "./version.ts";

describe("parseVersion", () => {
  test("解析 x.y.z 与预发布后缀", () => {
    expect(parseVersion("1.2.3")).toEqual([1, 2, 3]);
    expect(parseVersion(" 0.9.12 ")).toEqual([0, 9, 12]);
    expect(parseVersion("2.0.0-beta.1")).toEqual([2, 0, 0]);
  });

  test("不是三段数字就返回 null", () => {
    expect(parseVersion("1.2")).toBeNull();
    expect(parseVersion("v1.2.3")).toBeNull();
    expect(parseVersion("abc")).toBeNull();
  });
});

describe("bump", () => {
  test("三档各自重置低位", () => {
    expect(bump("1.2.3", "patch")).toBe("1.2.4");
    expect(bump("1.2.3", "minor")).toBe("1.3.0");
    expect(bump("1.2.3", "major")).toBe("2.0.0");
  });

  test("当前版本不合规时报错", () => {
    expect(() => bump("v1.2.3", "patch")).toThrow();
  });
});

describe("resolveTarget", () => {
  test("认关键词与显式版本", () => {
    expect(resolveTarget("0.1.0", "minor")).toBe("0.2.0");
    expect(resolveTarget("0.1.0", "0.5.0")).toBe("0.5.0");
  });

  test("看不懂的参数报错", () => {
    expect(() => resolveTarget("0.1.0", "latest")).toThrow();
  });
});

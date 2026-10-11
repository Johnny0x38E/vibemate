import { expect, test } from "vitest";
import { deriveServerName, serverNameForAttempt } from "./deriveServerName";

test("deriveServerName lowercases ASCII words and joins separators", () => {
    expect(deriveServerName("My Tools")).toBe("my-tools");
    expect(deriveServerName("  Brave_Search  ")).toBe("brave-search");
});

test("deriveServerName falls back when the name has no portable characters", () => {
    expect(deriveServerName("工具箱")).toMatch(/^server-[0-9a-f]{8}$/);
});

test("serverNameForAttempt appends numeric suffixes within length bounds", () => {
    expect(serverNameForAttempt("tools", 0)).toBe("tools");
    expect(serverNameForAttempt("tools", 1)).toBe("tools-2");
    const long = "a".repeat(64);
    expect(serverNameForAttempt(long, 2).length).toBeLessThanOrEqual(64);
});

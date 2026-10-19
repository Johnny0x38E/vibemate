import { expect, test } from "vitest";
import { lineDiff } from "./lineDiff";

function compact(before: string, after: string) {
    return lineDiff(before, after)?.map((line) => `${line.kind}:${line.text}`);
}

test("reports unchanged, removed and added lines", () => {
    expect(compact("a\nb\nc", "a\nc\nd")).toEqual([
        "same:a",
        "removed:b",
        "same:c",
        "added:d",
    ]);
});

test("keeps line numbers on each side", () => {
    const lines = lineDiff("one\ntwo", "one\nthree\nfour");
    expect(lines).toEqual([
        { kind: "same", text: "one", oldLine: 1, newLine: 1 },
        { kind: "removed", text: "two", oldLine: 2, newLine: null },
        { kind: "added", text: "three", oldLine: null, newLine: 2 },
        { kind: "added", text: "four", oldLine: null, newLine: 3 },
    ]);
});

test("handles empty sides and empty lines", () => {
    expect(compact("", "a\n\nb")).toEqual(["added:a", "added:", "added:b"]);
    expect(compact("a\n\nb", "")).toEqual([
        "removed:a",
        "removed:",
        "removed:b",
    ]);
    expect(lineDiff("", "")).toEqual([]);
});

test("refuses a pathologically large comparison explicitly", () => {
    expect(lineDiff("a\nb\nc", "d\ne\nf", 4)).toBeNull();
});

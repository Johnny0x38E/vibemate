import { expect, test } from "vitest";
import { nearestTokenStop, tokenStopLabel } from "./tokenStops";

test("snaps catalog token counts onto the shared stops", () => {
    expect(nearestTokenStop(null)).toBeNull();
    expect(nearestTokenStop(0)).toBeNull();
    expect(nearestTokenStop(128_000)).toBe(128 * 1024);
    expect(nearestTokenStop(1_048_576)).toBe(1024 * 1024);
    // 393216 sits closer to 500×1024 than to 200×1024.
    expect(nearestTokenStop(393_216)).toBe(500 * 1024);
    expect(tokenStopLabel(1_048_576)).toBe("1M");
    expect(tokenStopLabel(null)).toBeNull();
});

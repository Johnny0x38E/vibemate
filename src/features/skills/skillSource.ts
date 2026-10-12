import type { SkillRecord } from "../../lib/desktop/skills";

/**
 * Where a skill's content is tracked from, so both the list row and the detail
 * page state the same thing.
 *
 * - `address`: an upstream source is recorded, which is what a later update
 *   check would follow.
 * - `local`: the content came from a folder on this device and no upstream
 *   source is known.
 * - `unknown`: neither is recorded, which no import produces today.
 *
 * A future install flow records the address it downloaded from (for example
 * `skills.sh`), so it only changes what the `address` branch carries.
 */
export type SkillSource =
    | { kind: "address"; url: string; revision: string | null }
    | { kind: "local" }
    | { kind: "unknown" };

/** Decide which source line a skill shows. */
export function skillSource(record: SkillRecord): SkillSource {
    if (record.sourceUrl !== null) {
        return {
            kind: "address",
            url: record.sourceUrl,
            revision: record.sourceRevision,
        };
    }
    if (record.originPath !== null) {
        return { kind: "local" };
    }
    return { kind: "unknown" };
}

/** One line of a line-level diff between the current and retained text. */
export interface DiffLine {
    kind: "same" | "added" | "removed";
    text: string;
    /** 1-based line in the current text; null for an added line. */
    oldLine: number | null;
    /** 1-based line in the retained text; null for a removed line. */
    newLine: number | null;
}

/**
 * Bounded longest-common-subsequence line diff.
 *
 * The complete current and retained text can each be up to the configuration
 * read limit. The table is capped so a pathological pair cannot exhaust memory;
 * `null` means the caller must show the explicit "too large" notice instead of
 * claiming a complete diff. Text is already sanitized by Rust.
 */
export function lineDiff(
    before: string,
    after: string,
    maxCells = 2_000_000,
): DiffLine[] | null {
    const oldLines = splitLines(before);
    const newLines = splitLines(after);
    const width = newLines.length + 1;
    if ((oldLines.length + 1) * width > maxCells) return null;

    // Suffix LCS lengths: table[i * width + j] is the LCS of oldLines[i..] and
    // newLines[j..]. A typed array keeps the bounded case inexpensive.
    const table = new Uint32Array((oldLines.length + 1) * width);
    for (let i = oldLines.length - 1; i >= 0; i -= 1) {
        for (let j = newLines.length - 1; j >= 0; j -= 1) {
            const here = i * width + j;
            table[here] =
                oldLines[i] === newLines[j]
                    ? (table[(i + 1) * width + j + 1] ?? 0) + 1
                    : Math.max(
                          table[(i + 1) * width + j] ?? 0,
                          table[i * width + j + 1] ?? 0,
                      );
        }
    }

    const result: DiffLine[] = [];
    let i = 0;
    let j = 0;
    while (i < oldLines.length && j < newLines.length) {
        if (oldLines[i] === newLines[j]) {
            result.push({
                kind: "same",
                text: oldLines[i] ?? "",
                oldLine: i + 1,
                newLine: j + 1,
            });
            i += 1;
            j += 1;
            continue;
        }
        const skipOld = table[(i + 1) * width + j] ?? 0;
        const skipNew = table[i * width + j + 1] ?? 0;
        if (skipOld >= skipNew) {
            result.push({
                kind: "removed",
                text: oldLines[i] ?? "",
                oldLine: i + 1,
                newLine: null,
            });
            i += 1;
        } else {
            result.push({
                kind: "added",
                text: newLines[j] ?? "",
                oldLine: null,
                newLine: j + 1,
            });
            j += 1;
        }
    }
    while (i < oldLines.length) {
        result.push({
            kind: "removed",
            text: oldLines[i] ?? "",
            oldLine: i + 1,
            newLine: null,
        });
        i += 1;
    }
    while (j < newLines.length) {
        result.push({
            kind: "added",
            text: newLines[j] ?? "",
            oldLine: null,
            newLine: j + 1,
        });
        j += 1;
    }
    return result;
}

/** Split preserving empty inner lines; empty text has no lines. */
function splitLines(text: string): string[] {
    return text === "" ? [] : text.split("\n");
}

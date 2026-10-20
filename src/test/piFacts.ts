import type { PiModelFact } from "../lib/desktop/pi";

/**
 * A complete Pi fact list in the order Rust builds it: one row for each item,
 * with a written, a kept and a not-stated example so every shape is covered.
 */
export const piModelFactsFixture: PiModelFact[] = [
    {
        item: "contextWindow",
        source: "library",
        state: "written",
        target: "contextWindow",
        value: "128000",
    },
    {
        item: "maxOutputTokens",
        source: "library",
        state: "written",
        target: "maxTokens",
        value: "64000",
    },
    {
        item: "reasoning",
        source: "catalog",
        state: "written",
        target: "reasoning",
        value: "true",
    },
    {
        item: "imageInput",
        source: "notStated",
        state: "notStated",
        target: "input",
        value: null,
    },
    {
        item: "thinkingLevel",
        source: "choice",
        state: "unchanged",
        target: "modelThinkingLevels",
        value: "low",
    },
];

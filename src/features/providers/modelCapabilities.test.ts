import { expect, test } from "vitest";
import type {
    CapabilityEvidence,
    CapabilityOverrides,
    ModelCapabilities,
} from "../../lib/desktop/models";
import {
    effectiveCapabilityState,
    hasCapabilityDeclarations,
    supportedModelCapabilities,
} from "./modelCapabilities";

const catalog: ModelCapabilities = {
    thinking: {
        state: "supported",
        source: "openrouter_model_reasoning",
    },
    imageInput: { state: "unknown", source: null },
    toolCalls: {
        state: "unsupported",
        source: "openrouter_model_supported_parameters",
    },
};

const noDeclarations: CapabilityOverrides = {
    thinking: null,
    imageInput: null,
    toolCalls: null,
};

test("only affirmatively supported capabilities produce model-row icons", () => {
    expect(supportedModelCapabilities(catalog, noDeclarations)).toEqual([
        "thinking",
    ]);
});

test("user declarations override the catalog in both directions", () => {
    expect(
        supportedModelCapabilities(catalog, {
            thinking: "unsupported",
            imageInput: "supported",
            toolCalls: null,
        }),
    ).toEqual(["imageInput"]);
    expect(hasCapabilityDeclarations(noDeclarations)).toBe(false);
    expect(
        hasCapabilityDeclarations({
            ...noDeclarations,
            toolCalls: "supported",
        }),
    ).toBe(true);
});

test("a user declaration wins over the catalog evidence for display", () => {
    const unknown: CapabilityEvidence = { state: "unknown", source: null };
    expect(effectiveCapabilityState(catalog.thinking, null)).toBe("supported");
    expect(effectiveCapabilityState(catalog.thinking, "unsupported")).toBe(
        "unsupported",
    );
    expect(effectiveCapabilityState(unknown, "supported")).toBe("supported");
    expect(effectiveCapabilityState(unknown, null)).toBe("unknown");
});

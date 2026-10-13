import type {
    CapabilityEvidence,
    CapabilityOverride,
    CapabilityOverrides,
    CapabilityState,
    ModelCapabilities,
} from "../../lib/desktop/models";

export type ModelCapabilityName = "thinking" | "imageInput" | "toolCalls";

/** No catalog evidence at all: every capability is unknown. */
export const UNKNOWN_CAPABILITIES: ModelCapabilities = {
    thinking: { state: "unknown", source: null },
    imageInput: { state: "unknown", source: null },
    toolCalls: { state: "unknown", source: null },
};

/**
 * Nothing is user-declared. Used for rows that cannot carry declarations (an
 * upstream catalog browse) and as the "restore default" value of the editor.
 */
export const NO_CAPABILITY_DECLARATIONS: CapabilityOverrides = {
    thinking: null,
    imageInput: null,
    toolCalls: null,
};

/**
 * The state a capability has for display: a user declaration always wins over
 * the catalog evidence, so an unknown catalog result can still be filled in.
 */
export function effectiveCapabilityState(
    evidence: CapabilityEvidence,
    declaration: CapabilityOverride | null,
): CapabilityState {
    return declaration ?? evidence.state;
}

/** Whether anything is user-declared, so "restore default" has work to do. */
export function hasCapabilityDeclarations(
    overrides: CapabilityOverrides,
): boolean {
    return (
        overrides.thinking !== null ||
        overrides.imageInput !== null ||
        overrides.toolCalls !== null
    );
}

/** The user declaration for one capability, or null when it follows the catalog. */
export function capabilityDeclaration(
    overrides: CapabilityOverrides,
    name: ModelCapabilityName,
): CapabilityOverride | null {
    switch (name) {
        case "thinking":
            return overrides.thinking;
        case "imageInput":
            return overrides.imageInput;
        case "toolCalls":
            return overrides.toolCalls;
    }
}

/**
 * Return only affirmatively declared capabilities, preserving unknown as absent.
 * A user declaration of "not supported" hides a catalog claim of support.
 */
export function supportedModelCapabilities(
    capabilities: ModelCapabilities,
    overrides: CapabilityOverrides,
): ModelCapabilityName[] {
    const entries: [ModelCapabilityName, CapabilityEvidence][] = [
        ["thinking", capabilities.thinking],
        ["imageInput", capabilities.imageInput],
        ["toolCalls", capabilities.toolCalls],
    ];
    const names: ModelCapabilityName[] = [];
    for (const [name, evidence] of entries) {
        const declaration = capabilityDeclaration(overrides, name);
        if (effectiveCapabilityState(evidence, declaration) === "supported")
            names.push(name);
    }
    return names;
}

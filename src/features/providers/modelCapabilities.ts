import type { ModelCapabilities } from "../../lib/desktop/models";

export type ModelCapabilityName = "thinking" | "imageInput" | "toolCalls";

/** Return only affirmatively declared capabilities, preserving unknown as absent. */
export function supportedModelCapabilities(
    capabilities: ModelCapabilities,
): ModelCapabilityName[] {
    const names: ModelCapabilityName[] = [];
    if (capabilities.thinking.state === "supported") names.push("thinking");
    if (capabilities.imageInput.state === "supported") names.push("imageInput");
    if (capabilities.toolCalls.state === "supported") names.push("toolCalls");
    return names;
}

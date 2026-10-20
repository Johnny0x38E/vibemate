import type { ProviderModel } from "../lib/desktop/models";
import type { ClaudeEffortPreference } from "../lib/desktop/claudePreference";

/** Non-secret model facts for preference boundary and async-editor tests. */
export function effortModelFixture(
    providerId = "0123456789abcdef0123456789abcdef",
    modelId = "deepseek-flash",
): ProviderModel {
    return {
        providerId,
        modelId,
        source: "fetched",
        selected: true,
        alias: null,
        upstreamName: null,
        contextWindow: null,
        maxOutputTokens: null,
        inputModalities: null,
        outputModalities: null,
        supportedEndpoints: null,
        capabilities: {
            thinking: { state: "unknown", source: null },
            imageInput: { state: "unknown", source: null },
            toolCalls: { state: "unknown", source: null },
        },
        capabilityOverrides: {
            thinking: "supported",
            imageInput: null,
            toolCalls: null,
        },
        thinkingEffort: {
            kind: "listed",
            levels: ["low", "high", "max"],
            defaultLevel: "high",
        },
        routeSupport: "supported",
        resolvedProtocol: "anthropic_messages",
        upstreamState: "listed",
        lastSeenAtMs: null,
        missingSinceMs: null,
        createdAtMs: 1,
        updatedAtMs: 1,
    };
}

/** A first read has no preference row even when the model declares a default. */
export function effortPreferenceFixture(
    model: ProviderModel = effortModelFixture(),
): ClaudeEffortPreference {
    return {
        snapshot: { model, contentDigest: "1".repeat(64) },
        revision: null,
        selectedEffort: null,
    };
}

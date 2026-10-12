import {
    AGENT_CONFIG_FILES,
    AGENT_IDS,
    type AgentObservation,
    type DiscoveryInput,
} from "../lib/desktop/agents";

/** Empty window-local inputs shared by discovery boundary and UI tests. */
export const automaticAgentPaths: DiscoveryInput = {
    claudePath: null,
    codexPath: null,
    opencodePath: null,
    piPath: null,
    grokPath: null,
    hermesPath: null,
};

/** Build a complete synthetic report, replacing only explicitly supplied entries. */
export function agentReport(
    overrides: AgentObservation[] = [],
): AgentObservation[] {
    return AGENT_IDS.map(
        (id) =>
            overrides.find((row) => row.id === id) ?? {
                id,
                state: "notFound",
                source: "none",
                entryPath: null,
                resolvedPath: null,
                version: null,
                versionEvidence: "none",
                capabilityEvidence: "unverified",
                configDirectory: `/home/test/config/${id}`,
                directorySource: "default",
                files: AGENT_CONFIG_FILES[id].map((name) => ({
                    name,
                    state: "missing",
                })),
            },
    );
}

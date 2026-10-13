import { expect, test } from "vitest";
import { supportedModelCapabilities } from "./modelCapabilities";

test("only explicitly supported declarations produce model-row icons", () => {
    expect(
        supportedModelCapabilities({
            thinking: {
                state: "supported",
                source: "openrouter_model_reasoning",
            },
            imageInput: { state: "unknown", source: null },
            toolCalls: {
                state: "unsupported",
                source: "openrouter_model_supported_parameters",
            },
        }),
    ).toEqual(["thinking"]);
});

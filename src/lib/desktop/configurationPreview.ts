import { invoke, isTauri } from "@tauri-apps/api/core";
import * as z from "zod";
import { isSafePath, type ClaudeCodePlan } from "./agents";
import type { PiPlan } from "./pi";
import type { GrokPlan } from "./grok";
import {
    summarySchema,
    textSchema,
    VersionRestoreRequestError,
} from "./versionRestore";

/** The already-reviewed native model plan; never a frontend-assembled document. */
export type ModelConfigurationPlan = ClaudeCodePlan | PiPlan | GrokPlan;
const fileSchema = z.strictObject({
    targetPath: z.string().refine(isSafePath),
    summary: summarySchema,
    text: textSchema.nullable(),
});
/** Summary plus optional complete sanitized text for one native file. */
export type ModelPreviewFile = z.infer<typeof fileSchema>;

/** Read summaries, or one selected file's full text, bound to the entire plan.
 * Browser preview never invents data. Reject malformed/mismatched responses as
 * a whole, without retaining IPC payloads or schema diagnostics in errors.
 */
export async function readModelConfigurationPreview(
    plan: ModelConfigurationPlan,
    entryPath: string | null,
    fileIndex: number | null = null,
): Promise<
    { kind: "preview" } | { kind: "desktop"; files: ModelPreviewFile[] }
> {
    if (!isTauri()) return { kind: "preview" };
    const targets =
        plan.agent === "pi"
            ? plan.files.map((file) => file.target)
            : plan.agent === "grokBuild"
              ? [plan.file.target]
              : [plan.target];
    if (
        fileIndex !== null &&
        (!Number.isInteger(fileIndex) ||
            fileIndex < 0 ||
            fileIndex >= targets.length)
    ) {
        throw new VersionRestoreRequestError("invalid_request");
    }
    let value: unknown;
    try {
        value = await invoke<unknown>("read_model_configuration_preview", {
            input: {
                agent: plan.agent,
                providerId: plan.source.providerId,
                modelId: plan.source.modelId,
                entryPath,
                reviewed: plan,
                fileIndex,
            },
        });
    } catch (error: unknown) {
        throw new VersionRestoreRequestError(
            error === "plan_stale" || error === "invalid_request"
                ? error
                : "operation_failed",
        );
    }
    const parsed = z.array(fileSchema).min(1).max(2).safeParse(value);
    if (
        !parsed.success ||
        parsed.data.length !== targets.length ||
        parsed.data.some((file, index) => {
            const expectedFormat = plan.agent === "grokBuild" ? "toml" : "json";
            return (
                file.targetPath !== targets[index]?.path ||
                file.summary.format !== expectedFormat ||
                (index === fileIndex
                    ? file.text === null ||
                      file.text.revisionId !== `file-${String(index)}` ||
                      file.text.targetPath !== file.targetPath ||
                      file.text.format !== expectedFormat
                    : file.text !== null)
            );
        })
    ) {
        throw new VersionRestoreRequestError("invalid_response");
    }
    return { kind: "desktop", files: parsed.data };
}

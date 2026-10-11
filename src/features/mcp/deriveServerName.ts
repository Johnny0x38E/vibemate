/** Portable MCP server identifier derived from a display name (Pi namespace rules). */
export function deriveServerName(displayName: string): string {
    const trimmed = displayName.trim();
    let slug = "";
    for (const char of trimmed) {
        if (/[a-zA-Z0-9]/.test(char)) {
            slug += char.toLowerCase();
        } else if (/[\s_./\\-]/.test(char)) {
            slug += "-";
        }
    }
    slug = slug.replace(/-+/g, "-").replace(/^-|-$/g, "");
    if (slug.length > 64) {
        slug = slug.slice(0, 64).replace(/-$/, "");
    }
    if (slug.length === 0) {
        slug = `server-${randomSuffix()}`;
    }
    return slug;
}

/** Retry suffix when the derived identifier collides with an existing definition. */
export function serverNameForAttempt(base: string, attempt: number): string {
    if (attempt <= 0) return base;
    const suffix = `-${String(attempt + 1)}`;
    const maxBase = Math.max(1, 64 - suffix.length);
    const trimmed = base.slice(0, maxBase).replace(/-$/, "");
    return `${trimmed}${suffix}`;
}

function randomSuffix(): string {
    const bytes = new Uint8Array(4);
    crypto.getRandomValues(bytes);
    return [...bytes]
        .map((byte) => byte.toString(16).padStart(2, "0"))
        .join("");
}
